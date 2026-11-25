#![cfg(test)]
//! Portable transport regressions using the production upstream handshake and routing.
use super::*;
use tungstenite::client::IntoClientRequest;
use tungstenite::error::CapacityError;

fn listener() -> TcpListener {
    TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap()
}

fn timed(stream: TcpStream) -> TcpStream {
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    stream
}

#[test]
fn official_sized_plugin_catalog_reaches_the_authenticated_client_through_routing() {
    // Measured official plugin/list response: one final text frame, below 64 MiB.
    const OFFICIAL_BYTES: usize = 48_300_053;
    let official = listener();
    let official_addr = official.local_addr().unwrap();
    let official_thread = std::thread::spawn(move || {
        let (stream, _) = official.accept().unwrap();
        let mut ws = tungstenite::accept(timed(stream)).unwrap();
        let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(
            request,
            json!({"id":2,"method":"plugin/list","params":{"cwds":[]}})
        );
        let mut response = String::from(r#"{"id":2,"result":{"data":[{"description":""#);
        let suffix = r#""}],"nextCursor":null}}"#;
        response.extend(std::iter::repeat_n(
            'x',
            OFFICIAL_BYTES - response.len() - suffix.len(),
        ));
        response.push_str(suffix);
        assert_eq!(response.len(), OFFICIAL_BYTES);
        // tungstenite sends this Message as a single frame, without fragmentation.
        ws.send(Message::text(response)).unwrap();
    });
    let local = listener();
    let local_addr = local.local_addr().unwrap();
    let home = std::env::temp_dir().join(format!("ivy-large-catalog-{}", nonce().unwrap()));
    std::fs::create_dir(&home).unwrap();
    let relay_home = home.clone();
    let relay = std::thread::spawn(move || {
        let (stream, _) = local.accept().unwrap();
        let mut client = authenticated(stream, "catalog-test-capability").unwrap();
        // The production handshake is the regression boundary: replacing it with
        // the old default constructor must reject OFFICIAL_BYTES before routing.
        let mut upstream =
            connect_upstream(timed(TcpStream::connect(official_addr).unwrap())).unwrap();
        let mut routing = Routing {
            registry: Registry::for_home(&relay_home).unwrap(),
            pending: HashMap::new(),
            internal: HashMap::new(),
            active: None,
            exit_route: Arc::new(Mutex::new(RouteExit::default())),
            last_thread: None,
            epoch: "catalog-epoch".into(),
            pane_id: "catalog-pane".into(),
            pane_dir: relay_home.clone(),
            launch_dir: relay_home,
            serial: 0,
        };
        let request_deadline = Instant::now() + Duration::from_secs(15);
        let request = loop {
            assert!(Instant::now() < request_deadline, "local request timed out");
            match client.read() {
                Ok(Message::Text(text)) => break serde_json::from_str(&text).unwrap(),
                Err(tungstenite::Error::Io(e))
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                other => panic!("expected authenticated plugin/list request: {other:?}"),
            }
        };
        routing
            .request(request, &mut upstream, &mut client)
            .unwrap();
        let response = upstream
            .read()
            .expect("official-sized upstream frame must decode");
        assert_eq!(response.len(), OFFICIAL_BYTES);
        let value = serde_json::from_str(response.to_text().unwrap()).unwrap();
        routing.response(value, &mut upstream, &mut client).unwrap();
        assert!(routing.pending.is_empty());
        assert!(routing.internal.is_empty());
        assert!(routing.active.is_none());
    });
    let mut request = format!("ws://{local_addr}/").into_client_request().unwrap();
    request.headers_mut().insert(
        "authorization",
        "Bearer catalog-test-capability".parse().unwrap(),
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME));
    let (mut client, _) = tungstenite::client::client_with_config(
        request,
        timed(TcpStream::connect(local_addr).unwrap()),
        Some(config),
    )
    .unwrap();
    client
        .send(Message::text(
            r#"{"id":2,"method":"plugin/list","params":{"cwds":[]}}"#,
        ))
        .unwrap();
    let response = client.read().expect("catalog must reach the local client");
    assert_eq!(response.len(), OFFICIAL_BYTES);
    let value: Value = serde_json::from_str(response.to_text().unwrap()).unwrap();
    assert_eq!(value["id"], 2);
    assert_eq!(value["result"]["nextCursor"], Value::Null);
    let description = value["result"]["data"][0]["description"].as_str().unwrap();
    assert_eq!(description.len(), OFFICIAL_BYTES - 65);
    assert!(description.bytes().all(|b| b == b'x'));
    relay.join().unwrap();
    official_thread.join().unwrap();
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn upstream_rejects_an_over_envelope_frame_from_its_header() {
    let official = listener();
    let address = official.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = official.accept().unwrap();
        let mut ws = tungstenite::accept(timed(stream)).unwrap();
        // Only ten bytes; no payload allocation or transmission is necessary.
        let mut header = vec![0x81, 127];
        header.extend_from_slice(&((MAX_FRAME as u64) + 1).to_be_bytes());
        ws.get_mut().write_all(&header).unwrap();
    });
    let mut upstream = connect_upstream(timed(TcpStream::connect(address).unwrap())).unwrap();
    assert!(
        matches!(upstream.read(), Err(tungstenite::Error::Capacity(CapacityError::MessageTooLong { size, max_size })) if size == MAX_FRAME + 1 && max_size == MAX_FRAME)
    );
    server.join().unwrap();
}

#[test]
fn upstream_rejects_a_fragmented_message_over_the_existing_envelope() {
    let official = listener();
    let address = official.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = official.accept().unwrap();
        let mut ws = tungstenite::accept(timed(stream)).unwrap();
        // Reuse a 1 MiB fixture: individual frames fit, but the aggregate does not.
        // The decoder, rather than the fixture, owns the bounded accumulated message.
        let payload = vec![b'x'; 1 << 20];
        for index in 0..65 {
            let mut header = vec![if index == 0 { 0x01 } else { 0x00 }, 127];
            header.extend_from_slice(&(payload.len() as u64).to_be_bytes());
            ws.get_mut().write_all(&header).unwrap();
            ws.get_mut().write_all(&payload).unwrap();
        }
    });
    let mut upstream = connect_upstream(timed(TcpStream::connect(address).unwrap())).unwrap();
    assert!(
        matches!(upstream.read(), Err(tungstenite::Error::Capacity(CapacityError::MessageTooLong { size, max_size })) if size == MAX_FRAME + (1 << 20) && max_size == MAX_FRAME)
    );
    server.join().unwrap();
}
