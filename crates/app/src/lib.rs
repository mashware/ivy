//! Ventana, divisiones y pintado.
//!
//! La única caja atada al framework de interfaz. Si se cambia de framework, se reescribe esta y
//! las otras dos siguen tal cual.
//!
//! Es librería además de binario a propósito: el banco de barrido que mide el repintado tiene que
//! llamar al pintor de verdad, y si lo duplicara su número dejaría de ser el del producto.
//!
//! Por ahora la ventana abre vacía. Lo que va llegando después: la tabla de color, la traducción
//! de teclado, el puente con el pseudoterminal, el pintor y los cronómetros.

pub mod tema;

use eframe::egui;

/// El nombre con el que la ventana se presenta al escritorio.
const NOMBRE: &str = "agent-terminal";

/// El fondo del proyecto. Hoy es el color de la ventana entera; cuando llegue el pintor pasará a
/// ser el de las celdas que no traigan fondo propio, y con él se irá a la tabla de color.
/// Privado a propósito hasta entonces: no hay ningún consumidor fuera de esta caja al que valga la
/// pena dejarle atarse a una constante que va a mudarse de sitio.
const FONDO: egui::Color32 = egui::Color32::from_rgb(0x1E, 0x1E, 0x1E);

/// Abre la ventana y no vuelve hasta que se cierra.
pub fn arrancar() -> eframe::Result {
    eframe::run_native(
        NOMBRE,
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(Ventana))),
    )
}

/// La ventana raíz. Todavía no guarda nada porque no hay nada que guardar.
struct Ventana;

impl eframe::App for Ventana {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Vacía a propósito: este hito solo comprueba que la ventana abre, cierra y no cuesta
        // nada en reposo. Lo que se dibuje aquí es el pintor, y llega con su propia medida.
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // El fondo se pinta aquí y no con un panel dentro de `ui` para que también cubra el
        // primer frame y los huecos de un redimensionado, donde todavía no ha corrido ningún
        // panel: sin esto asoma el gris por defecto de eframe.
        FONDO.to_normalized_gamma_f32()
    }
}
