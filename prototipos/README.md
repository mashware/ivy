# Prototipos

## `flow-panel`

El panel de estado funcionando, en Python, sobre WezTerm. **Es la implementación de referencia
del contrato** ([../docs/04-contrato-panel.md](../docs/04-contrato-panel.md)).

Copia de trabajo instalada en `~/.local/bin/flow-panel`. Esta es la del repositorio — si tocas
una, sincroniza la otra.

```
flow-panel [ruta]                 vista global: todos los trabajos del repo
flow-panel --focus [ruta]         solo el trabajo de la rama actual
flow-panel --work MT-1234 [ruta]  fijado a un trabajo concreto
flow-panel --json [ruta]          lista de trabajos abiertos (para el selector)
```

Atajos en WezTerm: `Ctrl+Alt+F` vista global · `Ctrl+Alt+G` elegir tarea y fijarla.

**Qué vale de aquí para la app:** el render adaptativo (ancho y alto), la degradación de la
barra de fases, los enlaces cortos con OSC 8, la alineación por bloques y el aviso de resumen
rancio. Todo eso hay que reimplementarlo, pero las decisiones ya están tomadas y probadas.

## `mockup-terminal.html`

Maqueta visual de la app: pestañas, divisiones, panel por división, contexto y coste,
indicador de actividad y plegado. Datos falsos, paleta real.

Sirve para juzgar proporciones antes de escribir Rust. Ábrelo en el navegador y toca los
controles de arriba.
