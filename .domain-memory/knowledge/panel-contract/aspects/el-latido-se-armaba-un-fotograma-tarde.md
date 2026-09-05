---
id: asp_el-latido-se-armaba-un-fotograma-tarde_qn6jme
slug: el-latido-se-armaba-un-fotograma-tarde
name: >-
  El latido se armaba con el estado del fotograma anterior, y en una ventana en
  reposo «un fotograma tarde» significa «nunca»
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T13:22:53.529Z'
updated_at: '2026-08-13T13:22:53.529Z'
last_verified: '2026-08-13T13:22:53.529Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/panel.rs
symbols:
  - poner_el_despertador
  - cuanto_falta_para_avisar
  - hasta_el_siguiente_salto
content_hashes:
  crates/app/src/lib.rs: 'sha256:1e1fccb7584128dc015d57b4a85b7afba61479eddebd8cffdbabeba928cc19be'
  crates/app/src/panel.rs: 'sha256:568e67aba57fd9f60e9ca802161c515776057b5d550f6074bf1af4cf561d98a8'
tags:
  - egui
  - eframe
  - bucle de ventana
  - repaint
  - F1.6
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**El fallo.** En eframe 0.36.1 la mitad `logic` corre **antes** que `ui` en la misma pasada, y los paneles se releen en `ui`. Armando el despertador solo en `logic`, la alarma se calculaba con el estado del fotograma **anterior**: en el fotograma en que un panel se lee por primera vez todavía no hay panel, así que no hay nada que programar. Medido con sonda: el retardo publicado era `Duration::MAX` y **la ventana se dormía para siempre**. Pasa en la primera publicación de la sesión y en **cada** publicación de un productor más lento que su propio plazo.

> **En un bucle de modo inmediato partido en dos mitades, cualquier cosa que se calcule a partir de estado que la otra mitad muta llega un fotograma tarde. Y en una ventana reactiva «un fotograma tarde» es «nunca»: no hay fotograma siguiente si nadie lo pide.**

**Por qué hacen falta las dos llamadas, y no vale con mover la única al final.** `Context::run_logic` —leído en la fuente— **empieza poniendo `repaint_delay = Duration::MAX`**, y es la única mitad que corre con la ventana **minimizada u ocluida**. Sin la llamada en `logic`, cada vuelta de ventana tapada borra la alarma y nadie la rearma: el aviso de un productor muerto no volvería a aparecer **ni al desminimizar**. Sin la de `ui`, no se ve el panel recién leído. Hay una prueba por cada una, y la segunda hizo falta añadirla en la validación: quitar la de `logic` dejaba las 286 en verde.

**El hermano pequeño del mismo fallo: el número congelado.** «no news for 5 min» se quedaba en 5 durante horas, porque una vez cumplido el plazo ya nadie pedía repintado. Un panel viejo tiene que **seguir** pidiendo despertador para el momento en que su número vaya a cambiar — al minuto siguiente, o a la hora siguiente cuando el aviso ya se cuenta en horas.

**Coste acotado por construcción:** un despertar cuando un panel cumple su plazo, y después uno por minuto mientras siga viejo. Con la ventana en reposo y ningún panel viejo, la ventana sigue durmiendo.

## Where it lives

`Ventana::poner_el_despertador` en `crates/app/src/lib.rs`, llamada desde **las dos** mitades del bucle de eframe 0.36.1 (`logic` y el final de `ui`). El número sale de `Estado::cuanto_falta_para_avisar` en `crates/app/src/panel.rs`.
