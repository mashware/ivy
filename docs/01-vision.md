# Visión

## Qué es

Un terminal que hospeda agentes. Tres niveles:

```
Ventana
└── Pestañas
    └── Divisiones          ← cada una es una consola independiente
        ├── Terminal        ← donde corre el agente (o lo que sea)
        └── Panel           ← estado de esa tarea, alimentado desde fuera
```

El panel no lo escribe la app. Lo escribe **quien esté haciendo el trabajo**, mediante un
fichero con formato definido. La app solo lo pinta.

## Para quién

Gente que trabaja con varios agentes de terminal en paralelo y necesita saber, de un vistazo:

- cuál está trabajando y cuál la espera
- por qué paso va cada tarea
- a cuál le queda poco contexto
- cuánto lleva gastado cada una
- qué está bloqueado y por qué

## Qué lo diferencia

**El contrato es abierto.** El panel de opencode solo sabe de opencode: sus sesiones, sus
tokens, sus herramientas. Es un buen panel cerrado. Aquí el panel recibe lo mismo de quien sea,
lo que permite tener cuatro divisiones con cuatro productores distintos y una vista global
que los resume.

**Preserva las suscripciones.** Cualquier integración por API ata a un proveedor y factura
tokens aparte. El terminal es el único punto donde todos los agentes convergen ya autenticados
con lo que el usuario paga. Ver [02-decisiones.md](02-decisiones.md) → Control por consola.

## Qué NO es

- **No es un agente.** No razona, no llama a modelos, no tiene opinión sobre el trabajo.
- **No es un multiplexor de sesiones remotas.** Eso es tmux/Zellij y no compite con ellos.
- **No reescribe el emulador de terminal.** Ese motor se consume como librería.

## Principios

1. **La app no sabe de dominios.** No conoce `flow`, ni git, ni tickets. Solo el contrato.
2. **Quien hace el trabajo publica el estado.** La app nunca deduce ni parsea para adivinar.
3. **Que funcione sin el panel.** Si nadie publica nada, sigue siendo un terminal correcto.
4. **La información que no se puede deducir desde fuera vale más que la que sí.** Contexto
   restante, coste, bloqueos: eso solo lo sabe el agente, y es lo que hay que facilitarle publicar.
