//! Optional user-manager placement; every bus future has an owned finite deadline.
use crate::termination::CachedChild;
use futures_lite::{StreamExt, future};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, Value},
};

const DESTINATION: &str = "org.freedesktop.systemd1";
const MANAGER: &str = "/org/freedesktop/systemd1";
const INTERFACE: &str = "org.freedesktop.systemd1.Manager";
const OBSERVATION: Duration = Duration::from_millis(500);

pub(crate) struct Scope {
    connection: Connection,
    name: String,
    path: OwnedObjectPath,
}
async fn bounded<T>(
    deadline: Instant,
    operation: impl std::future::Future<Output = zbus::Result<T>>,
) -> zbus::Result<T> {
    future::or(operation, async move {
        async_io::Timer::at(deadline).await;
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "scope manager deadline expired",
        )
        .into())
    })
    .await
}
async fn manager(connection: &Connection) -> zbus::Result<Proxy<'_>> {
    zbus::proxy::Builder::new(connection)
        .destination(DESTINATION)?
        .path(MANAGER)?
        .interface(INTERFACE)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}
// systemd's bus_unit_set_properties reports unknown transient properties as PropertyReadOnly;
// accept that exact optional-property diagnostic, never authorization or malformed-value errors.
fn unsupported_oom_property(name: &str, message: Option<&str>) -> bool {
    match (name, message) {
        ("org.freedesktop.DBus.Error.UnknownProperty", Some(message)) => {
            message.contains("OOMPolicy")
        }
        ("org.freedesktop.DBus.Error.PropertyReadOnly", Some(message)) => {
            message == "Cannot set property OOMPolicy, or unknown property."
        }
        _ => false,
    }
}
pub(crate) fn place(
    pid: u32,
    name: &str,
    identity: &str,
    deadline: Instant,
) -> crate::Result<Scope> {
    future::block_on(bounded(deadline, async {
        let connection = Connection::session().await?;
        let proxy = manager(&connection).await?;
        let _: () = proxy.call("Subscribe", &()).await?;
        let mut jobs = proxy.receive_signal("JobRemoved").await?;
        let description = format!("Ivy console {identity}");
        let properties = |oom| {
            let mut properties = vec![
                ("Description", Value::from(description.as_str())),
                ("Slice", Value::from("app.slice")),
                ("PIDs", Value::from(vec![pid])),
            ];
            if oom {
                properties.push(("OOMPolicy", Value::from("kill")));
            }
            properties
        };
        let mut selected_name = name.to_owned();
        let auxiliary: Vec<(&str, Vec<(&str, Value<'_>)>)> = Vec::new();
        let job: OwnedObjectPath = match proxy
            .call(
                "StartTransientUnit",
                &(name, "fail", properties(true), &auxiliary),
            )
            .await
        {
            Ok(job) => job,
            Err(zbus::Error::MethodError(error, message, _))
                if unsupported_oom_property(error.as_str(), message.as_deref()) =>
            {
                // A rejected transient request can leave a stub unit until manager GC runs.
                // A distinct owned name avoids racing that GC when retrying without OOMPolicy.
                selected_name = format!("{}-basic.scope", name.trim_end_matches(".scope"));
                proxy
                    .call(
                        "StartTransientUnit",
                        &(
                            selected_name.as_str(),
                            "fail",
                            properties(false),
                            &auxiliary,
                        ),
                    )
                    .await?
            }
            Err(error) => return Err(error),
        };
        let mut settled = false;
        while let Some(signal) = jobs.next().await {
            let (_, path, unit, result): (u32, OwnedObjectPath, String, String) =
                signal.body().deserialize()?;
            if path == job && unit == selected_name {
                if result != "done" {
                    return Err(std::io::Error::other(format!("scope job failed: {result}")).into());
                }
                settled = true;
                break;
            }
        }
        if !settled {
            return Err(std::io::Error::other("scope job stream closed before settlement").into());
        }
        let path: OwnedObjectPath = proxy.call("GetUnit", &(selected_name.as_str(),)).await?;
        // Retain the unit before the gated process may exit and race unit garbage collection.
        let _: () = proxy.call("RefUnit", &(selected_name.as_str(),)).await?;
        let scope = Scope {
            connection: connection.clone(),
            name: selected_name,
            path,
        };
        let unit = Proxy::new(
            &connection,
            DESTINATION,
            scope.path.as_str(),
            "org.freedesktop.systemd1.Scope",
        )
        .await?;
        let group: String = unit.get_property("ControlGroup").await?;
        let actual = std::fs::read_to_string(format!("/proc/{pid}/cgroup"))?;
        if !placed_in(&actual, &group) {
            return Err(std::io::Error::other("scope placement was not confirmed").into());
        }
        Ok(scope)
    }))
    .map_err(Into::into)
}
fn placed_in(actual: &str, group: &str) -> bool {
    !group.is_empty()
        && actual.lines().any(|line| {
            let mut fields = line.splitn(3, ':');
            let hierarchy = fields.next();
            let controllers = fields.next();
            let path = fields.next();
            path == Some(group)
                && (hierarchy == Some("0") && controllers == Some("")
                    || controllers.is_some_and(|controllers| {
                        controllers.split(',').any(|name| name == "name=systemd")
                    }))
        })
}
impl Scope {
    pub(crate) fn observe(self, child: CachedChild) -> Arc<AtomicBool> {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        child.observe(true, false);
        std::thread::spawn(move || {
            future::block_on(async {
                let mut oom = false;
                while !stopping.load(Ordering::Relaxed) {
                    let deadline = child.exited().map(|exit| exit + OBSERVATION);
                    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        break;
                    }
                    let operation_end =
                        deadline.unwrap_or_else(|| Instant::now() + Duration::from_millis(100));
                    let result = bounded(operation_end, async {
                        let proxy: Proxy<'_> = zbus::proxy::Builder::new(&self.connection)
                            .destination(DESTINATION)?
                            .path(self.path.as_str())?
                            .interface("org.freedesktop.systemd1.Scope")?
                            .cache_properties(zbus::proxy::CacheProperties::No)
                            .build()
                            .await?;
                        proxy.get_property::<String>("Result").await
                    })
                    .await;
                    match result {
                        Ok(result) if result == "oom-kill" => {
                            oom = true;
                            break;
                        }
                        Err(_) => break,
                        _ => {}
                    }
                    // Bounded by the root-exit observation window and checked cancellation.
                    async_io::Timer::after(if deadline.is_some() {
                        Duration::from_millis(10)
                    } else {
                        Duration::from_millis(100)
                    })
                    .await;
                }
                child.observe(false, oom);
            });
            drop(self);
        });
        stop
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let connection = self.connection.clone();
        let name = self.name.clone();
        // No manager calls or joins on the drawing thread, including startup error cleanup.
        std::thread::spawn(move || {
            let _ = future::block_on(bounded(
                Instant::now() + Duration::from_millis(100),
                async {
                    let proxy = manager(&connection).await?;
                    let _: () = proxy.call("UnrefUnit", &(name.as_str(),)).await?;
                    let _: () = proxy.call("ResetFailedUnit", &(name.as_str(),)).await?;
                    Ok(())
                },
            ));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_requires_the_exact_unified_or_systemd_hierarchy() {
        assert!(placed_in(
            "0::/app.slice/ivy-console-test.scope\n",
            "/app.slice/ivy-console-test.scope"
        ));
        assert!(placed_in(
            "1:name=systemd:/app.slice/ivy-console-test.scope\n",
            "/app.slice/ivy-console-test.scope"
        ));
        assert!(!placed_in(
            "0::/app.slice/ivy-console-other.scope\n",
            "/app.slice/ivy-console-test.scope"
        ));
        assert!(!placed_in(
            "3:memory:/app.slice/ivy-console-test.scope\n",
            "/app.slice/ivy-console-test.scope"
        ));
        assert!(!placed_in("0::/\n", ""));
    }
    #[test]
    fn only_unknown_optional_oom_property_permits_a_basic_scope_retry() {
        assert!(unsupported_oom_property(
            "org.freedesktop.DBus.Error.PropertyReadOnly",
            Some("Cannot set property OOMPolicy, or unknown property.")
        ));
        assert!(!unsupported_oom_property(
            "org.freedesktop.DBus.Error.AccessDenied",
            Some("OOMPolicy")
        ));
        assert!(!unsupported_oom_property(
            "org.freedesktop.DBus.Error.InvalidArgs",
            Some("OOMPolicy")
        ));
        assert!(!unsupported_oom_property(
            "org.freedesktop.DBus.Error.PropertyReadOnly",
            Some("Cannot set property PIDs, or unknown property.")
        ));
    }
    #[test]
    fn a_stalled_manager_future_is_cancelled_at_its_owned_deadline() {
        let started = Instant::now();
        let result: zbus::Result<()> = future::block_on(bounded(
            started + Duration::from_millis(20),
            std::future::pending(),
        ));
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn an_expired_deadline_does_not_poll_a_stalled_future_forever() {
        let result: zbus::Result<()> =
            future::block_on(bounded(Instant::now(), std::future::pending()));
        assert!(result.is_err());
    }
}
