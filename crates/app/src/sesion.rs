//! El formato con el que la disposición de la ventana va y vuelve de disco, y lo que se le exige.
//!
//! **Aquí no hay ni una consola.** Este módulo describe formas y las valida; materializarlas —que es
//! bifurcar un proceso por hoja— ocurre en la raíz, que es donde viven los campos de una división. La
//! frontera no es de gusto: mantiene esta mitad **pura y probable sin abrir un pseudoterminal**, y deja
//! el `fork` en un solo sitio.
//!
//! **Y los tipos de aquí son un espejo, no los de la disposición viva.** Cambiarle un campo al árbol de
//! [`crate::Nodo`] no debe cambiar lo que hay escrito en el disco de nadie; el precio es una conversión
//! de ida y otra de vuelta, y lo que compra es que las dos formas evolucionen por separado. La única
//! excepción, y por su razón, es el peso: ver [`Hijo::peso`].

use serde::{Deserialize, Serialize};

/// Cuántas hojas se aceptan en un documento entero.
///
/// **Existe aunque D14 descartara ponerle techo al número de divisiones, y no la contradice.** Partir
/// exige un gesto humano y la geometría **se niega antes de crear nada**; restaurar bifurca N shells
/// **antes de que exista geometría** —al abrir no se sabe el tamaño de la ventana—, así que el suelo que
/// protege a partir está **aguas abajo del `fork`**. Es techo **del documento**, no de divisiones.
///
/// **Y el número se elige contra el recurso que guarda, no contra lo que cabe en pantalla.** Cada hoja
/// restaurada es un proceso, un pseudoterminal, sus descriptores y ~1,06 MiB de colas: un millar de hojas
/// —lo que deja pasar el techo de bytes— serían más de un gigabyte de colas antes de pintar nada, o sea
/// el techo convertido en el desastre que venía a evitar. 128 son unos 136 MiB, y siguen estando por
/// encima de las del orden de cien consolas que caben en la ventana con la que se mide.
pub const HOJAS_MAXIMAS: usize = 128;

/// La versión del formato.
///
/// **Es obligatoria, al contrario que la del contrato del panel, y por la razón contraria.** Allí puede
/// faltar porque hay ficheros anteriores al campo, escritos por terceros; aquí no existe ninguno, porque
/// este fichero lo escribe siempre la aplicación.
///
/// Sube **solo con lo que un lector viejo malinterpretaría** —una clave renombrada, quitada, o con otro
/// significado—; **añadir una clave opcional no la sube**, y que se quede en 1 mucho tiempo es la señal
/// de que funciona. Va **una vez por documento y nunca por nodo**: versiones anidadas dejarían mezclar
/// formas dentro de un mismo fichero.
pub const VERSION: u64 = 1;

/// Lo más grande que se acepta interpretar.
///
/// **Está aquí y no en quien lea el fichero, y eso es la decisión.** D16 compromete este techo, y si
/// viviera en el camino que abre el fichero, quien escriba **otro** camino que interprete un documento
/// tendría que acordarse por su cuenta. En el módulo que posee el formato no hay nada que recordar.
///
/// **64 KiB y no los 256 del contrato del panel**: el documento legítimo más grande que este formato
/// produce son unos 27 KiB —el techo de hojas por lo que ocupa cada una—, así que esto es 2,4 veces eso.
/// Y la razón de que haya techo no es el pico de interpretar: es que **el fichero tiene tamaño
/// arbitrario, y leerlo sin tope es reservar sin tope en el arranque**.
pub const BYTES_MAXIMOS: usize = 64 * 1024;

/// El peso más pequeño que se acepta; ver [`PESO_MAXIMO`].
pub const PESO_MINIMO: f32 = 1e-6;

/// El peso más grande que se acepta.
///
/// **El rango existe porque el reparto MULTIPLICA dos pesos, y eso no se ve mirando un peso a la vez.**
/// Al cerrar una división, las nietas suben a hermanas con el peso escalado —`w · suyo / suma`—, y el
/// producto de dos pesos válidos puede **subdesbordar a cero**: comprobado ejecutándolo,
/// `1·10⁻⁴⁰ · 1·10⁻⁴⁰ / 2·10⁻⁴⁰` da **exactamente cero**, y un peso cero es lo que [`crate::Peso`] existe
/// para hacer imposible. Un fichero puede traer esos números; **ningún gesto los produce**.
///
/// **Y el número sale de la multiplicación, no del gusto.** Con los pesos dentro de este rango, el
/// producto de dos cualesquiera cae entre 10⁻¹² y 10¹², holgadamente representable. Los que los gestos
/// producen van del orden de una centésima a la centena —el reparto de fábrica y el suelo del 1 % que el
/// arrastre respeta—, así que el rango los contiene con seis órdenes de margen a cada lado.
pub const PESO_MAXIMO: f32 = 1e6;

/// Lo que se guarda de una consola, y es todo lo que se guarda de ella.
///
/// De los quince campos de una división viva, dos. Los otros trece no, y cada uno por su razón: el
/// nombre porque el contador que garantiza que no se repitan es de la ventana y un nombre viejo chocaría
/// con él; el directorio del panel y su estado porque una consola restaurada **estrena identidad**; el
/// pseudoterminal y su pantalla porque son un proceso vivo y un historial; y los cronómetros porque son
/// de la corrida.
///
/// **Y ningún campo de texto libre, que es una decisión y no una casualidad.** Ni programa, ni
/// argumentos, ni entorno, ni directorio de trabajo, ni ninguna ruta. El lanzamiento de una consola deja
/// de ser inofensivo en cuanto el programa o sus argumentos **procedan de configuración**, y este fichero
/// es configuración: si el tipo no lo tiene, no hay nada que conectar a un `spawn`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Hoja {
    /// En celdas, como en la división viva: lo que el panel tiene que conservar al cambiar el aumento
    /// del escritorio es **cuántos caracteres caben**, no cuántos puntos ocupa.
    pub ancho_del_panel: usize,
    /// Aparte del ancho a propósito: plegar no pierde el ancho de trabajo, y desplegar vuelve **al que
    /// tenía** en vez de inventarse uno.
    pub plegado: bool,
}

/// En qué sentido reparte una rama.
///
/// **Es un tipo propio y no [`crate::Orientacion`], a propósito.** Los nombres de este enum son **el
/// formato**: aparecen literalmente en el fichero de todo el que use la aplicación. Reutilizar el tipo
/// vivo ataría el fichero a un nombre que el código puede querer cambiar mañana por razones que no
/// tienen nada que ver con el disco. El precio son cuatro líneas de conversión.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientacion {
    /// Una al lado de otra; se reparte el ancho.
    Columnas,
    /// Una encima de otra; se reparte el alto.
    Filas,
}

/// Una consola, o un corte con sus partes.
///
/// **Etiqueta explícita —`hoja` o `rama`— y no «se deduce de qué clave está presente».** La forma sin
/// etiqueta se lee igual de bien y **pierde el motivo del fallo**: quien intente resolver la disyuntiva
/// probando formas devuelve «no encaja en ninguna», y lo que hace falta decir es **qué** clave sobra o
/// falta y **dónde**. La etiqueta la escribe `serde` de serie, así que el motivo sale gratis.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Nodo {
    Hoja(Hoja),
    Rama {
        orientacion: Orientacion,
        hijos: Vec<Hijo>,
    },
}

/// Un hijo de una rama, con lo que le toca del reparto.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Hijo {
    /// **Un número suelto, y no el tipo del peso vivo, y esto sí es deliberado.** Derivar la lectura
    /// sobre [`crate::Peso`] **se saltaría su constructor**, que es lo único que hace imposible una suma
    /// de pesos igual a cero — el `NaN` en un rectángulo que ese tipo existe para evitar. Así que el
    /// cable lleva `f32` y la validación lo pasa por `Peso::nuevo`: **una sola implementación de la
    /// invariante**, la que ya estaba probada.
    pub peso: f32,
    pub nodo: Nodo,
}

/// Una pestaña.
///
/// **Un objeto con una sola clave en vez del nodo suelto**, para poder añadirle algo opcional mañana sin
/// subir la versión. No lleva ni el foco ni el nombre: la ventana abre en la primera pestaña, y los
/// nombres no se reutilizan.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Pestaña {
    pub raiz: Nodo,
}

/// El documento entero, tal como se lee del disco: **sin validar todavía**.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Documento {
    /// La versión del formato. La clave es corta porque aparece en todos los ficheros y no dice nada
    /// más de lo que dice.
    pub v: u64,
    pub pestañas: Vec<Pestaña>,
}

/// Un documento que **ya pasó por la validación**, y por eso se puede materializar.
///
/// **El campo es privado y no hay más forma de construirlo que [`validar`]**, así que «este árbol está
/// bien formado» es propiedad del tipo y no de que alguien se acuerde de comprobarlo antes. Es la misma
/// doctrina que [`crate::Peso`] un nivel más arriba, y por el mismo motivo: este proyecto tiene contado
/// con números lo que pasa con las invariantes que dependen de la memoria de quien escribe.
#[derive(Clone, PartialEq, Debug)]
pub struct Valido {
    documento: Documento,
    /// **La cuenta que hizo la validación, guardada en vez de recalculada.** Recorrer el árbol otra vez
    /// para volver a contar lo mismo tiene dos costes y solo uno es el trabajo: el otro es que **el
    /// número que se comprobó contra el techo y el que lee quien llama podrían dejar de ser el mismo**.
    hojas: usize,
}

impl Valido {
    /// Las pestañas del documento, en orden.
    pub fn pestañas(&self) -> &[Pestaña] {
        &self.documento.pestañas
    }

    /// Cuántas consolas va a haber que abrir. Se sabe **antes** de abrir ninguna.
    pub fn hojas(&self) -> usize {
        self.hojas
    }
}

/// Por qué un documento no sirve.
///
/// **La tolerancia es por documento y no por campo, al contrario que en el contrato del panel.** Aquel
/// lo escribe un agente a mano y perder el panel entero por un campo que nadie mira sería lo peor
/// posible; este lo escribe siempre la aplicación, así que si no se entiende, **abrir de fábrica es una
/// respuesta correcta y barata** y adivinar media disposición es peor que no restaurarla.
///
/// Cada variante lleva **qué** y **dónde**, porque el único canal para decirlo es una línea.
#[derive(Clone, PartialEq, Debug)]
pub enum Problema {
    /// No es JSON, o no encaja en la forma. Trae el motivo tal como lo da el intérprete, que ya nombra
    /// la clave y la posición.
    NoSeEntiende(String),
    /// La versión no es la que este binario conoce. **Qué hacer con eso no se decide aquí**: una más
    /// nueva no se puede sobrescribir y una más vieja habría que leerla con su lector, y las dos cosas
    /// son del camino que toca el fichero.
    Version { hallada: u64 },
    /// Un documento sin pestañas no describe ninguna ventana.
    SinPestañas,
    /// Una rama con menos de dos hijos. **La invariante que sostiene la razón de ser del árbol**: si una
    /// rama puede tener una sola hija, vuelve el anidamiento que hace que una frontera arrastre un
    /// bloque de dos consolas, que es lo que más se le critica a `tmux`.
    RamaDeUnHijo { pestaña: usize },
    /// Dos ramas del mismo sentido, una dentro de otra. La otra mitad de la misma invariante: partir
    /// inserta como hermana justamente para no construir esto.
    RamasAnidadas { pestaña: usize },
    /// Un peso que el tipo del peso vivo rechaza —cero, negativo, o no un número— o que se sale del
    /// rango que el reparto puede multiplicar.
    ///
    /// **No lleva el valor, y eso lo decidió una revisión.** Llevándolo, este tipo se comparaba mal
    /// consigo mismo: un peso `NaN` guardado aquí hace que `PesoInvalido != PesoInvalido`, porque `NaN`
    /// no es igual a nada, ni a sí mismo. El síntoma ya estaba en las pruebas —la de los pesos era **la
    /// única** obligada a comprobar solo la variante mientras sus vecinas comparaban el problema
    /// entero—, así que dejaba de atar **cuál** peso se rechazó. El valor va en el mensaje, que es donde
    /// lo lee quien tiene que arreglarlo.
    PesoInvalido { pestaña: usize },
    /// Los pesos de una rama son cada uno un número **y su suma no lo es**.
    ///
    /// **No es «los pesos tienen que sumar algo concreto», que se descartó a propósito**: son
    /// proporciones, y `{3, 1}` y `{0,5, 1,5}` describen el mismo reparto. Es que el reparto **divide
    /// cada peso por la suma de su rama**, así que con una suma infinita a **todas** les toca cero — dos
    /// pesos de 3·10³⁸ son cada uno finito y positivo, y sumados desbordan. La otra dirección **no hace
    /// falta comprobarla**: dos números positivos no pueden sumar cero.
    SumaQueNoEsUnNumero { pestaña: usize },
    /// Más consolas de las que se aceptan abrir de golpe.
    DemasiadasHojas { cuantas: usize },
    /// Más bytes de los que se aceptan interpretar.
    DemasiadoGrande { bytes: usize },
}

impl std::fmt::Display for Problema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSeEntiende(motivo) => write!(f, "no se entiende: {motivo}"),
            Self::Version { hallada } => {
                write!(
                    f,
                    "lo escribió la versión {hallada} del formato y esta conoce la {VERSION}"
                )
            }
            Self::SinPestañas => write!(f, "no trae ninguna pestaña"),
            Self::RamaDeUnHijo { pestaña } => {
                write!(f, "la pestaña {pestaña} tiene un corte con un solo hijo")
            }
            Self::RamasAnidadas { pestaña } => write!(
                f,
                "la pestaña {pestaña} anida dos cortes del mismo sentido, que es la forma que hace \
                 que un divisor arrastre dos consolas"
            ),
            Self::PesoInvalido { pestaña } => write!(
                f,
                "la pestaña {pestaña} trae un peso que no puede serlo, o fuera del rango \
                 [{PESO_MINIMO:e}, {PESO_MAXIMO:e}] que el reparto puede multiplicar"
            ),
            Self::SumaQueNoEsUnNumero { pestaña } => write!(
                f,
                "la pestaña {pestaña} tiene un corte cuyos pesos son números y su suma no: repartir \
                 dejaría a todas sus consolas con cero de ancho"
            ),
            Self::DemasiadasHojas { cuantas } => write!(
                f,
                "pide {cuantas} consolas y no se abren más de {HOJAS_MAXIMAS} de golpe"
            ),
            Self::DemasiadoGrande { bytes } => write!(
                f,
                "ocupa {bytes} bytes y no se interpretan más de {BYTES_MAXIMOS}"
            ),
        }
    }
}

/// Lo que dice un documento, si dice algo que se pueda construir.
pub fn interpretar(texto: &str) -> Result<Valido, Problema> {
    // **El techo se aplica ANTES de interpretar, no después.** Interpretar reserva el árbol entero, así
    // que comprobar el tamaño al final es no tener techo: el gasto ya ocurrió.
    if texto.len() > BYTES_MAXIMOS {
        return Err(Problema::DemasiadoGrande { bytes: texto.len() });
    }

    let documento: Documento =
        serde_json::from_str(texto).map_err(|fallo| Problema::NoSeEntiende(fallo.to_string()))?;
    validar(documento)
}

/// Lo que hay que escribir para volver a leer esto, o **nada** si no se pudo.
///
/// **Devuelve `Option` y no una cadena, y eso lo decidió una revisión.** La primera versión devolvía
/// cadena vacía al fallar, y el camino que escribe el fichero **no habría podido distinguirla de un
/// documento**: escribe un temporal y lo renombra encima del bueno, así que un fallo aquí habría
/// **destruido la disposición guardada** y dejado cero bytes en su sitio. Abrir de fábrica es la
/// respuesta a un fichero que no se entiende; **borrar el que sí se entendía no es respuesta a nada.**
/// Con `None`, quien escribe se salta el renombrado y el fichero anterior sobrevive.
pub fn escribir(documento: &Documento) -> Option<String> {
    // Sin sangrar: nadie lo edita a mano —lo escribe siempre la aplicación— y una línea por consola
    // hace el fichero varias veces más grande sin que nadie lo agradezca.
    match serde_json::to_string(documento) {
        Ok(texto) => Some(texto),
        Err(fallo) => {
            // Inalcanzable con los tipos de aquí —no hay claves que no sean texto ni números que no
            // puedan serlo—, así que se dice y no se calla: un fallo silencioso en el camino que guarda
            // es el que se descubre al reabrir y no encontrar nada.
            eprintln!("no se pudo escribir la disposición ({fallo})");
            None
        }
    }
}

/// El único camino que produce un [`Valido`].
///
/// **Se recorre el árbol entero antes de aceptar nada**, y en particular se cuentan las hojas **antes**
/// de que se abra la primera: comprobar el techo después del `fork` es no tener techo.
pub fn validar(documento: Documento) -> Result<Valido, Problema> {
    if documento.v != VERSION {
        return Err(Problema::Version {
            hallada: documento.v,
        });
    }
    if documento.pestañas.is_empty() {
        return Err(Problema::SinPestañas);
    }

    // **Un solo recorrido, que valida y cuenta.** Dos —uno para revisar y otro para contar— visitan cada
    // nodo dos veces en el camino del arranque, y además abren la puerta a que el número que se compara
    // con el techo no sea el que luego se lee.
    let mut hojas = 0;
    for (cual, pestaña) in documento.pestañas.iter().enumerate() {
        hojas += revisar(&pestaña.raiz, None, cual)?;
    }

    if hojas > HOJAS_MAXIMAS {
        return Err(Problema::DemasiadasHojas { cuantas: hojas });
    }

    Ok(Valido { documento, hojas })
}

/// Las invariantes de forma, rama por rama.
///
/// `de_fuera` es el sentido de la rama que contiene a ésta, o nada en la raíz: es lo único que hace
/// falta para cazar el anidamiento, porque partir solo puede crearlo entre una rama y su madre directa.
fn revisar(nodo: &Nodo, de_fuera: Option<Orientacion>, pestaña: usize) -> Result<usize, Problema> {
    let Nodo::Rama { orientacion, hijos } = nodo else {
        return Ok(1);
    };

    if hijos.len() < 2 {
        return Err(Problema::RamaDeUnHijo { pestaña });
    }
    if de_fuera == Some(*orientacion) {
        return Err(Problema::RamasAnidadas { pestaña });
    }

    let mut hojas = 0;
    for hijo in hijos {
        // **El peso pasa por el constructor del tipo vivo, y ADEMÁS por el rango.** Lo primero, porque es
        // la única forma de que no haya dos implementaciones de la misma invariante — la de allí ya está
        // probada. Lo segundo, porque ese constructor mira **un** peso y el reparto **multiplica dos**:
        // ver [`PESO_MAXIMO`].
        let fuera_de_rango = hijo.peso < PESO_MINIMO || hijo.peso > PESO_MAXIMO;
        if crate::Peso::nuevo(hijo.peso).is_none() || fuera_de_rango {
            return Err(Problema::PesoInvalido { pestaña });
        }
        hojas += revisar(&hijo.nodo, Some(*orientacion), pestaña)?;
    }

    // **Y la suma, que es una invariante de la rama y no de cada peso.** Con el rango de arriba esto ya no
    // puede desbordar con dos hijos, pero se deja: el rango puede ampliarse mañana, y cuesta una suma.
    let suma: f32 = hijos.iter().map(|hijo| hijo.peso).sum();
    if !suma.is_finite() {
        return Err(Problema::SumaQueNoEsUnNumero { pestaña });
    }

    Ok(hojas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hoja(ancho: usize) -> Nodo {
        Nodo::Hoja(Hoja {
            ancho_del_panel: ancho,
            plegado: false,
        })
    }

    /// Una rama con esos pesos y una hoja por cada uno.
    fn rama(orientacion: Orientacion, pesos: &[f32]) -> Nodo {
        Nodo::Rama {
            orientacion,
            hijos: pesos
                .iter()
                .map(|peso| Hijo {
                    peso: *peso,
                    nodo: hoja(36),
                })
                .collect(),
        }
    }

    fn documento(raices: Vec<Nodo>) -> Documento {
        Documento {
            v: VERSION,
            pestañas: raices.into_iter().map(|raiz| Pestaña { raiz }).collect(),
        }
    }

    #[test]
    fn hoja_y_rama_se_distinguen_por_una_etiqueta_explicita() {
        // **Esto es el contrato, así que se afirma sobre el texto literal y no sobre el tipo.** Si un día
        // alguien cambia la representación del enum, la ida y vuelta seguiría funcionando y **el fichero
        // de todo el que ya use la aplicación dejaría de leerse**: es justo el fallo que la versión del
        // formato existe para no tener que arreglar por sorpresa.
        let escrito = escribir(&documento(vec![rama(Orientacion::Columnas, &[3.0, 1.0])]))
            .expect("un documento de dos hojas se escribe");

        assert_eq!(
            escrito,
            r#"{"v":1,"pestañas":[{"raiz":{"rama":{"orientacion":"columnas","hijos":[{"peso":3.0,"nodo":{"hoja":{"ancho_del_panel":36,"plegado":false}}},{"peso":1.0,"nodo":{"hoja":{"ancho_del_panel":36,"plegado":false}}}]}}}]}"#
        );
    }

    #[test]
    fn lo_escrito_se_vuelve_a_leer_igual() {
        let antes = documento(vec![
            Nodo::Rama {
                orientacion: Orientacion::Columnas,
                hijos: vec![
                    Hijo {
                        peso: 3.0,
                        nodo: hoja(52),
                    },
                    Hijo {
                        peso: 1.0,
                        nodo: Nodo::Rama {
                            orientacion: Orientacion::Filas,
                            hijos: vec![
                                Hijo {
                                    peso: 2.0,
                                    nodo: Nodo::Hoja(Hoja {
                                        ancho_del_panel: 36,
                                        plegado: true,
                                    }),
                                },
                                Hijo {
                                    peso: 1.0,
                                    nodo: hoja(24),
                                },
                            ],
                        },
                    },
                ],
            },
            hoja(36),
        ]);

        let escrito = escribir(&antes).expect("un documento válido se escribe");
        let despues = interpretar(&escrito).expect("lo que se acaba de escribir se lee");

        assert_eq!(despues.pestañas(), antes.pestañas.as_slice());
    }

    #[test]
    fn los_pesos_no_tienen_que_sumar_nada_concreto() {
        // Los pesos son **proporciones**, y el reparto normaliza por la suma de su rama: `{3, 1}` y
        // `{0,5, 1,5}` describen exactamente el mismo reparto. Exigir que sumaran uno o cien sería una
        // guarda sin escenario **y rompería la ida y vuelta** de cualquier árbol construido arrastrando,
        // que es de donde salen los pesos de verdad.
        let medio = documento(vec![rama(Orientacion::Filas, &[0.5, 1.5])]);
        let entero = documento(vec![rama(Orientacion::Filas, &[3.0, 9.0])]);

        assert!(validar(medio).is_ok());
        assert!(validar(entero).is_ok());
    }

    #[test]
    fn una_rama_con_un_solo_hijo_se_rechaza() {
        // **La invariante que sostiene la razón de ser del árbol.** Si una rama puede tener una sola hija,
        // vuelve el anidamiento que hace que una frontera arrastre un bloque de dos consolas — lo que más
        // se le critica a `tmux` y lo que D14 existe para evitar.
        let uno = documento(vec![rama(Orientacion::Columnas, &[1.0])]);

        assert_eq!(validar(uno), Err(Problema::RamaDeUnHijo { pestaña: 0 }));
    }

    #[test]
    fn dos_ramas_del_mismo_sentido_anidadas_se_rechazan() {
        let anidada = documento(vec![Nodo::Rama {
            orientacion: Orientacion::Columnas,
            hijos: vec![
                Hijo {
                    peso: 1.0,
                    nodo: hoja(36),
                },
                Hijo {
                    peso: 1.0,
                    nodo: rama(Orientacion::Columnas, &[1.0, 1.0]),
                },
            ],
        }]);

        assert_eq!(
            validar(anidada),
            Err(Problema::RamasAnidadas { pestaña: 0 })
        );
    }

    #[test]
    fn dos_ramas_de_sentidos_distintos_anidadas_se_aceptan() {
        // **El hermano de la prueba anterior, y hace falta.** Sin él, «rechaza el anidamiento» y «rechaza
        // cualquier rama dentro de otra» pasan las mismas pruebas, y la segunda haría imposible la
        // disposición en cuadro que F2.2 entrega.
        let cuadro = documento(vec![Nodo::Rama {
            orientacion: Orientacion::Columnas,
            hijos: vec![
                Hijo {
                    peso: 1.0,
                    nodo: rama(Orientacion::Filas, &[1.0, 1.0]),
                },
                Hijo {
                    peso: 1.0,
                    nodo: rama(Orientacion::Filas, &[1.0, 1.0]),
                },
            ],
        }]);

        assert!(validar(cuadro).is_ok());
    }

    #[test]
    fn un_peso_que_el_tipo_vivo_rechaza_se_rechaza_aqui() {
        // Cero y negativo llegan por JSON; `NaN` e infinito no tienen literal, así que la única forma de
        // que entren es un documento armado a mano — y aun así el constructor los para.
        for malo in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let documento = documento(vec![rama(Orientacion::Columnas, &[1.0, malo])]);

            assert!(
                matches!(validar(documento), Err(Problema::PesoInvalido { .. })),
                "un peso de {malo} tenía que caer"
            );
        }
    }

    #[test]
    fn el_techo_acepta_las_hojas_justas_y_rechaza_una_mas() {
        // **Las dos mitades, y la de abajo es la que importa.** Un techo probado solo por arriba no
        // distingue «rechaza más de 128» de «rechaza más de 1»: con la prueba de rechazo sola, poner el
        // techo en cualquier número más pequeño seguiría dando verde.
        let pesos: Vec<f32> = vec![1.0; HOJAS_MAXIMAS];
        let justas = documento(vec![rama(Orientacion::Columnas, &pesos)]);
        assert!(
            validar(justas).is_ok(),
            "{HOJAS_MAXIMAS} hojas tienen que caber"
        );

        let pesos: Vec<f32> = vec![1.0; HOJAS_MAXIMAS + 1];
        let una_mas = documento(vec![rama(Orientacion::Columnas, &pesos)]);
        assert_eq!(
            validar(una_mas),
            Err(Problema::DemasiadasHojas {
                cuantas: HOJAS_MAXIMAS + 1
            })
        );
    }

    #[test]
    fn el_techo_cuenta_las_hojas_de_todo_el_documento_y_no_de_una_pestaña() {
        // Si contara por pestaña, dos pestañas de 128 abrirían 256 consolas y el techo no habría servido
        // de nada: lo que se acota es cuántos procesos se bifurcan **al abrir**, y eso es de la ventana.
        let mitad: Vec<f32> = vec![1.0; HOJAS_MAXIMAS / 2 + 1];
        let dos = documento(vec![
            rama(Orientacion::Columnas, &mitad),
            rama(Orientacion::Columnas, &mitad),
        ]);

        assert!(matches!(
            validar(dos),
            Err(Problema::DemasiadasHojas { .. })
        ));
    }

    #[test]
    fn una_version_que_no_es_la_conocida_se_rechaza_diciendo_cual_es() {
        let del_futuro = Documento {
            v: VERSION + 1,
            pestañas: vec![Pestaña { raiz: hoja(36) }],
        };

        assert_eq!(
            validar(del_futuro),
            Err(Problema::Version {
                hallada: VERSION + 1
            })
        );
    }

    #[test]
    fn un_documento_sin_pestañas_se_rechaza() {
        let vacio = Documento {
            v: VERSION,
            pestañas: Vec::new(),
        };

        assert_eq!(validar(vacio), Err(Problema::SinPestañas));
    }

    #[test]
    fn lo_que_no_es_json_se_rechaza_y_se_dice_por_que() {
        // **Se afirma que el motivo llega, no solo que falla.** El único canal para contarlo es una línea,
        // así que un `Problema` sin texto dejaría al usuario con «no se pudo» y nada más.
        let Err(Problema::NoSeEntiende(motivo)) = interpretar("{esto no es json") else {
            panic!("un documento roto tiene que caer por no entenderse");
        };

        assert!(!motivo.is_empty(), "el motivo tiene que decir algo");
    }

    #[test]
    fn un_nodo_sin_etiqueta_no_se_lee() {
        // La forma que se descartó: hoja o rama deducidas de qué clave está presente. Si esto se leyera,
        // el fichero tendría dos formas válidas y una de ellas no sería la del contrato.
        let sin_etiqueta =
            r#"{"v":1,"pestañas":[{"raiz":{"ancho_del_panel":36,"plegado":false}}]}"#;

        assert!(matches!(
            interpretar(sin_etiqueta),
            Err(Problema::NoSeEntiende(_))
        ));
    }
}

#[cfg(test)]
mod tests_de_los_techos {
    use super::*;

    fn con_pesos(pesos: &[f32]) -> Documento {
        Documento {
            v: VERSION,
            pestañas: vec![Pestaña {
                raiz: Nodo::Rama {
                    orientacion: Orientacion::Columnas,
                    hijos: pesos
                        .iter()
                        .map(|peso| Hijo {
                            peso: *peso,
                            nodo: Nodo::Hoja(Hoja {
                                ancho_del_panel: 36,
                                plegado: false,
                            }),
                        })
                        .collect(),
                },
            }],
        }
    }

    #[test]
    fn dos_pesos_que_el_constructor_acepta_y_cuyo_producto_seria_cero_se_rechazan() {
        // **El agujero que una comprobación peso a peso no puede ver.** Al cerrar una división, las
        // nietas suben con el peso escalado —`w · suyo / suma`— y el producto de dos pesos válidos puede
        // subdesbordar: comprobado ejecutándolo, `1e-40 · 1e-40 / 2e-40` da **cero**, y un peso cero es lo
        // que `Peso` existe para hacer imposible. Un fichero puede traerlos; ningún gesto los produce.
        let diminuto = 1.0e-40_f32;
        assert!(
            crate::Peso::nuevo(diminuto).is_some(),
            "cada peso por separado tiene que ser válido, o esta prueba no prueba nada"
        );
        assert_eq!(
            diminuto * diminuto / (diminuto + diminuto),
            0.0,
            "y su escalado tiene que dar cero, o tampoco"
        );

        assert_eq!(
            validar(con_pesos(&[diminuto, diminuto])),
            Err(Problema::PesoInvalido { pestaña: 0 })
        );
    }

    #[test]
    fn dos_pesos_finitos_cuya_suma_no_es_un_numero_se_rechazan() {
        let al_limite = 3.0e38_f32;
        assert!(
            crate::Peso::nuevo(al_limite).is_some(),
            "cada peso por separado tiene que ser válido"
        );
        assert!(
            !(al_limite + al_limite).is_finite(),
            "y su suma tiene que desbordar"
        );

        // Con el rango, este cae por el rango antes que por la suma — y las dos cosas son correctas: lo
        // que importa es que **no se acepta**. La guarda de la suma sigue puesta porque el rango puede
        // ampliarse y ella no depende de él.
        assert!(matches!(
            validar(con_pesos(&[al_limite, al_limite])),
            Err(Problema::PesoInvalido { .. }) | Err(Problema::SumaQueNoEsUnNumero { .. })
        ));
    }

    #[test]
    fn los_extremos_del_rango_de_pesos_se_aceptan() {
        // **La mitad de aceptar, y hace falta.** Sin ella, poner el rango en `[1, 1]` pasaría la prueba
        // de rechazo y dejaría fuera cualquier disposición real.
        assert!(validar(con_pesos(&[PESO_MINIMO, PESO_MAXIMO])).is_ok());
        // Y lo que los gestos producen de verdad, con mucho margen dentro.
        assert!(validar(con_pesos(&[0.5, 1.5])).is_ok());
        assert!(validar(con_pesos(&[100.0, 0.01])).is_ok());
    }

    #[test]
    fn un_peso_fuera_del_rango_se_rechaza_por_los_dos_lados() {
        for malo in [PESO_MINIMO / 2.0, PESO_MAXIMO * 2.0] {
            assert_eq!(
                validar(con_pesos(&[1.0, malo])),
                Err(Problema::PesoInvalido { pestaña: 0 }),
                "un peso de {malo:e} tenía que caer"
            );
        }
    }

    #[test]
    fn un_documento_mas_grande_que_el_techo_se_rechaza_sin_interpretarlo() {
        // **Y se rechaza por el tamaño, no por la sintaxis.** El texto es basura: si el techo se
        // comprobara después de interpretar, el motivo sería «no se entiende» y el gasto ya habría
        // ocurrido. Que el motivo sea el tamaño **es** la prueba de que el orden es el correcto.
        let enorme = "x".repeat(BYTES_MAXIMOS + 1);

        assert_eq!(
            interpretar(&enorme),
            Err(Problema::DemasiadoGrande {
                bytes: BYTES_MAXIMOS + 1
            })
        );
    }

    #[test]
    fn un_documento_justo_del_tamaño_del_techo_se_interpreta() {
        // La otra mitad: el techo no puede estar en cero disfrazado. Se rellena con espacios, que JSON
        // ignora, hasta ocupar exactamente el techo.
        let minimo =
            r#"{"v":1,"pestañas":[{"raiz":{"hoja":{"ancho_del_panel":36,"plegado":false}}}]}"#;
        let relleno = " ".repeat(BYTES_MAXIMOS - minimo.len());
        let justo = format!("{minimo}{relleno}");
        assert_eq!(justo.len(), BYTES_MAXIMOS);

        assert!(interpretar(&justo).is_ok());
    }

    #[test]
    fn la_cuenta_de_hojas_que_se_publica_es_la_que_se_comparo_con_el_techo() {
        // Si `hojas()` recalculara en vez de leer lo guardado, podrían dejar de ser el mismo número.
        let dos_y_tres = Documento {
            v: VERSION,
            pestañas: vec![
                Pestaña {
                    raiz: Nodo::Rama {
                        orientacion: Orientacion::Columnas,
                        hijos: vec![
                            Hijo {
                                peso: 1.0,
                                nodo: Nodo::Hoja(Hoja {
                                    ancho_del_panel: 36,
                                    plegado: false,
                                }),
                            },
                            Hijo {
                                peso: 1.0,
                                nodo: Nodo::Hoja(Hoja {
                                    ancho_del_panel: 36,
                                    plegado: false,
                                }),
                            },
                        ],
                    },
                },
                Pestaña {
                    raiz: Nodo::Hoja(Hoja {
                        ancho_del_panel: 36,
                        plegado: false,
                    }),
                },
            ],
        };

        let valido = validar(dos_y_tres).expect("el documento es válido");
        assert_eq!(valido.hojas(), 3);
    }
}
