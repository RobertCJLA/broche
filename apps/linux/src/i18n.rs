// SPDX-License-Identifier: GPL-3.0-or-later
//! Interface languages. English is the key; Spanish is looked up from it.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Es,
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn get() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 { Lang::Es } else { Lang::En }
}

/// The language of the system (`LANG`, `LC_ALL`...), Spanish when it starts with `es`.
pub fn detect() -> Lang {
    let v = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    if v.to_lowercase().starts_with("es") { Lang::Es } else { Lang::En }
}

/// Translates `s` into the current language.
pub fn t(s: &str) -> String {
    if let Some(name) = s.strip_prefix("perm:") {
        return perm(name, get());
    }
    if get() == Lang::En {
        return s.to_string();
    }
    es(s).map_or_else(|| s.to_string(), str::to_string)
}

fn es(s: &str) -> Option<&'static str> {
    Some(match s {
        "Transferring files" => "Transfiriendo archivos",
        "File transfers" => "Transferencias de archivos",
        "failed transfer" => "transferencia fallida",
        "failed transfers" => "transferencias fallidas",
        "Waiting for approval" => "Esperando aprobación",
        "Waiting for the other device" => "Esperando al otro dispositivo",
        "Paused" => "En pausa",
        "Cancelled" => "Cancelada",
        "Transfer complete" => "Transferencia completada",
        "Transfer refused" => "Transferencia rechazada",
        "Device unreachable" => "No se puede contactar con el dispositivo",
        "Not enough storage space" => "No hay suficiente espacio de almacenamiento",
        "Transfer interrupted" => "Transferencia interrumpida",
        "Integrity check failed" => "Falló la comprobación de integridad",
        "files" => "archivos",
        "Downloading files" => "Descargando archivos",
        "Downloads complete" => "Descargas completadas",
        "Downloads finished with errors" => "Descargas finalizadas con errores",
        "files completed" => "archivos completados",
        "failed file" => "archivo fallido",
        "failed files" => "archivos fallidos",
        "Bluetooth calls" => "Llamadas por Bluetooth",
        "Use PC audio when answering" => "Usar el audio del PC al contestar",
        "Talk on this PC" => "Hablar desde este PC",
        "Audio on this PC" => "Audio en este PC",
        "Bluetooth calling is unavailable on this system." => {
            "Las llamadas por Bluetooth no están disponibles en este sistema."
        }
        "No Bluetooth phone connected." => "Ningún teléfono conectado por Bluetooth.",
        "Phone for calls" => "Teléfono para llamadas",
        "Choose a phone" => "Elige un teléfono",
        "Hear and answer calls on this PC" => "Escuchar y contestar llamadas en este PC",
        "This PC has no Bluetooth telephony (PipeWire with Bluetooth). Calls still show here, but their audio stays on the phone." => {
            "Este PC no tiene telefonía Bluetooth (PipeWire con Bluetooth). Las llamadas se siguen viendo aquí, pero su audio se queda en el teléfono."
        }
        "Ready: the phone is connected by Bluetooth." => "Listo: el teléfono está conectado por Bluetooth.",
        "Waiting for the phone to be paired and connected (step 2)." => {
            "Esperando a que el teléfono se empareje y conecte (paso 2)."
        }
        "Call audio travels over Bluetooth, so the phone and this PC must be paired. Broche never changes your Bluetooth or phone settings: you do each step yourself." => {
            "El audio de las llamadas viaja por Bluetooth, así que el teléfono y este PC deben estar emparejados. Broche nunca cambia tus ajustes de Bluetooth ni del teléfono: cada paso lo haces tú."
        }
        "1. On the phone: Broche › Settings › Permissions › Calls, contacts and messages › Allow. Android asks for Phone, Call log and Contacts so Broche can show who is calling and answer or hang up for you." => {
            "1. En el teléfono: Broche › Ajustes › Permisos › Llamadas, contactos y mensajes › Permitir. Android pide Teléfono, Registro de llamadas y Contactos para que Broche muestre quién llama y conteste o cuelgue por ti."
        }
        "2. On this PC: open the system Settings › Bluetooth and pair the phone." => {
            "2. En este PC: abre Configuración del sistema › Bluetooth y empareja el teléfono."
        }
        "3. On the phone, in that Bluetooth pairing, keep “Phone calls” on. You can turn off “Media audio” so music stays on the phone." => {
            "3. En el teléfono, en ese emparejamiento Bluetooth, deja activado «Llamadas telefónicas». Puedes desactivar «Audio multimedia» para que la música siga en el teléfono."
        }
        "To return call audio to the phone, use the audio selector on the phone's call screen." => {
            "Para devolver el audio al teléfono, usa el selector de audio de su pantalla de llamada."
        }
        "Pair your phone with this PC in Bluetooth settings to hear and answer calls on the PC." => {
            "Empareja el teléfono con este PC en los ajustes Bluetooth para escuchar y contestar llamadas desde el PC."
        }
        "Devices" => "Dispositivos",
        "Detailed permissions" => "Permisos detallados",
        "Technical log" => "Registro técnico",
        "History" => "Historial",
        "Go to a page, a setting or an action…" => "Ir a una página, un ajuste o una acción…",
        "Nothing matches." => "No hay coincidencias.",
        "Arrow keys to move · Enter to open · Esc to close" => {
            "Flechas para moverte · Intro para abrir · Esc para cerrar"
        }
        "A VPN is on" => "Hay una VPN activa",
        "Another copy of Broche may be using the same data, or the keyring didn't answer. Close other copies and open Broche again." => {
            "Puede que otra copia de Broche esté usando los mismos datos o que el llavero no haya respondido. Cierra las otras copias y abre Broche de nuevo."
        }
        "Broche's core is running" => "El núcleo de Broche está en marcha",
        "Broche's core isn't running" => "El núcleo de Broche no está en marcha",
        "Check" => "Revisar",
        "Check connection" => "Comprobar conexión",
        "Checking…" => "Comprobando…",
        "Connect it to Wi-Fi or a cable, on the same network as the phone." => {
            "Conéctalo a una red Wi-Fi o por cable, en la misma red que el teléfono."
        }
        "Connection check" => "Comprobar la conexión",
        "Copy command" => "Copiar la orden",
        "It can keep the phone and this PC apart. If they don't connect, try with the VPN off." => {
            "Puede separar el teléfono de este PC. Si no conectan, prueba con la VPN apagada."
        }
        "It works, but slower. On the same Wi-Fi it connects directly." => {
            "Funciona, pero más lento. En la misma red Wi-Fi conecta directamente."
        }
        "No firewall (firewalld) is blocking Broche" => "Ningún cortafuegos (firewalld) bloquea Broche",
        "No phone is paired" => "No hay ningún teléfono vinculado",
        "OK" => "Bien",
        "Open Broche on the phone and check it uses the same Wi-Fi. Some routers keep devices apart (client isolation)." => {
            "Abre Broche en el teléfono y comprueba que usa la misma red Wi-Fi. Algunos routers separan los dispositivos (aislamiento de clientes)."
        }
        "Pair one in Settings › Pair." => "Vincula uno en Ajustes › Vincular.",
        "Phones still find this PC. For this PC to find them too, run this in a terminal:" => {
            "Los teléfonos siguen encontrando este PC. Para que este PC también los encuentre, ejecuta esto en una terminal:"
        }
        "Problem" => "Problema",
        "Run this in a terminal (it asks for your password there):" => {
            "Ejecuta esto en una terminal (te pedirá la contraseña allí):"
        }
        "Seen {} s ago" => "Visto hace {} s",
        "The firewall blocks the phone" => "El cortafuegos bloquea el teléfono",
        "The firewall couldn't be checked" => "No se pudo comprobar el cortafuegos",
        "The firewall lets the phone connect" => "El cortafuegos deja conectar al teléfono",
        "This PC can't hear phones announce themselves" => "Este PC no oye los anuncios de los teléfonos",
        "This PC has no local network address" => "Este PC no tiene dirección en la red local",
        "This PC is on a local network" => "Este PC está en una red local",
        "What could keep a phone from reaching this PC, and how to fix it." => {
            "Qué podría impedir que un teléfono llegue a este PC, y cómo arreglarlo."
        }
        "Zone:" => "Zona:",
        "connected" => "conectado",
        "connected through the internet" => "conectado a través de internet",
        "not seen on this network" => "no se ve en esta red",
        "on this network but not connecting" => "está en esta red pero no conecta",
        "{} failed attempts" => "{} intentos fallidos",
        "Image" => "Imagen",
        "All" => "Todo",
        "Clips" => "Portapapeles",
        "Links" => "Enlaces",
        "Screen sessions" => "Sesiones de pantalla",
        "Search history" => "Buscar en el historial",
        "History couldn't be read." => "No se pudo leer el historial.",
        "Nothing in your history yet." => "Aún no hay nada en tu historial.",
        "Files, clips, links, photos and screen sessions with your phone show up here." => {
            "Aquí aparecen los archivos, portapapeles, enlaces, fotos y sesiones de pantalla con tu teléfono."
        }
        "Show more" => "Mostrar más",
        "(no title)" => "(sin título)",
        "Received from" => "Recibido de",
        "Sent to" => "Enviado a",
        "Just now" => "Ahora mismo",
        "{} min ago" => "hace {} min",
        "{} h ago" => "hace {} h",
        "{} days ago" => "hace {} días",
        "No entries yet. Warnings and errors show up here." => {
            "Todavía no hay entradas. Aquí aparecen los avisos y errores."
        }
        "Device" => "Dispositivo",
        "Video quality" => "Calidad de vídeo",
        "Normal · 720p, 30 fps" => "Normal · 720p, 30 fps",
        "High · 1080p, 60 fps (experimental)" => "Alta · 1080p, 60 fps (experimental)",
        "For the screens and the webcam. It is the same on the phone: change it on either. Applies to new streams." => {
            "Para las pantallas y la webcam. Es la misma en el teléfono: cámbiala en cualquiera de los dos. Se aplica a las transmisiones nuevas."
        }
        "High is smoother but uses more battery and network. Go back to Normal if the picture stutters." => {
            "Alta es más fluida pero gasta más batería y red. Vuelve a Normal si la imagen se entrecorta."
        }
        "Show only the phone" => "Mostrar solo el teléfono",
        "The phone's files are open in Broche. Open its window from the dock or with Alt+Tab." => {
            "Los archivos del teléfono están abiertos en Broche. Abre su ventana desde el dock o con Alt+Tab."
        }
        "Back to Broche" => "Volver a Broche",
        "Profile" => "Perfil",
        "Save" => "Guardar",
        "Custom: some permissions were changed by hand." => {
            "Personalizado: algunos permisos se cambiaron a mano."
        }
        "Notifications only" => "Solo notificaciones",
        "Notifications, messages, calls and contacts. Nothing else." => {
            "Notificaciones, mensajes, llamadas y contactos. Nada más."
        }
        "Basic" => "Básico",
        "Everyday sharing: files, clipboard, photos, music, and the phone's screen and camera. Nothing that controls this PC." => {
            "Lo de cada día: archivos, portapapeles, fotos, música y la pantalla y la cámara del teléfono. Nada que controle este PC."
        }
        "Multimedia" => "Multimedia",
        "Basic, plus browsing the phone's storage and seeing this PC's screen on the phone." => {
            "Básico, más explorar el almacenamiento del teléfono y ver la pantalla de este PC en el teléfono."
        }
        "Full control" => "Control total",
        "Everything, including this PC's mouse, keyboard and shortcut commands. Only for your own phone." => {
            "Todo, incluidos el ratón, el teclado y las órdenes de los atajos de este PC. Solo para tu propio teléfono."
        }
        "What may this phone do? ·" => "¿Qué puede hacer este teléfono? ·",
        "You can change it later in Settings › Device." => {
            "Puedes cambiarlo después en Ajustes › Dispositivo."
        }
        "Programs this app uses" => "Programas que usa la app",
        "Everything is installed." => "Está todo instalado.",
        "Some features need programs that aren't installed:" => {
            "Algunas funciones necesitan programas que no están instalados:"
        }
        "Install them with:" => "Instálalos con:",
        "Install them with your distribution's package manager." => {
            "Instálalos con el gestor de paquetes de tu distribución."
        }
        "Check again" => "Comprobar otra vez",
        "See the phone's screen and camera" => "Ver la pantalla y la cámara del teléfono",
        "Hear the phone's sound" => "Oír el sonido del teléfono",
        "Find my PC (ring)" => "Encontrar mi PC (timbre)",
        "Share this PC's screen" => "Compartir la pantalla de este PC",
        "Open apps from the shortcuts" => "Abrir apps desde los atajos",
        "Open links and folders" => "Abrir enlaces y carpetas",
        "Custom" => "Personalizados",
        "PC shortcuts on the phone" => "Atajos del PC en el teléfono",
        "Your shortcuts show on the Custom page of PC shortcuts on the phone. What they do stays on this PC. The phone needs the permission to control the mouse and keyboard, and for commands also the one to run commands." => {
            "Tus atajos aparecen en la página Personalizados de Atajos del PC en el teléfono. Lo que hacen se queda en este PC. El teléfono necesita el permiso para controlar el ratón y el teclado y, para las órdenes, también el de ejecutar órdenes."
        }
        "New button" => "Botón nuevo",
        "Button name" => "Nombre del botón",
        "Open an app" => "Abrir una app",
        "Open a link" => "Abrir un enlace",
        "Run a command" => "Ejecutar una orden",
        "Choose an app" => "Elige una app",
        "Program and arguments, e.g. notify-send Hello" => "Programa y argumentos, p. ej. notify-send Hola",
        "It runs without a shell: pipes, redirections and variables are plain text." => {
            "Se ejecuta sin shell: las tuberías, redirecciones y variables son texto normal."
        }
        "I understand the phone will be able to run this command on this PC" => {
            "Entiendo que el teléfono podrá ejecutar esta orden en este PC"
        }
        "Add button" => "Añadir botón",
        "Delete" => "Eliminar",
        "Couldn't save the shortcuts:" => "No se pudieron guardar los atajos:",
        "Couldn't update the shortcuts:" => "No se pudieron actualizar los atajos:",
        "Screen" => "Pantalla",
        "Messages" => "Mensajes",
        "Calls" => "Llamadas",
        "Contacts" => "Contactos",
        "Notifications" => "Notificaciones",
        "Photos" => "Fotos",
        "Pair" => "Vincular",
        "Activity" => "Actividad",
        "Language" => "Idioma",
        "Ready" => "Listo",
        "Clear" => "Limpiar",
        "Refresh" => "Actualizar",
        "Send" => "Enviar",
        "Open" => "Abrir",
        "Cancel" => "Cancelar",
        "Yes, it matches" => "Sí, coincide",
        "No" => "No",
        "Copy link" => "Copiar enlace",
        "Download" => "Descargar",
        "Stopped showing the phone's screen: Broche was minimized." => {
            "Se dejó de mostrar la pantalla del teléfono: Broche estaba minimizado."
        }
        "Stopped showing the phone's screen: nobody had looked at it for 3 minutes." => {
            "Se dejó de mostrar la pantalla del teléfono: nadie la miraba desde hacía 3 minutos."
        }
        "Island position" => "Posición de la isla",
        "Top left" => "Arriba a la izquierda",
        "Top centre" => "Arriba al centro (inicial)",
        "Top right" => "Arriba a la derecha",
        "Bottom" => "Abajo",
        "Lock the island's position" => "Bloquear la posición de la isla",
        "Otherwise drag it by its background to put it anywhere on the screen." => {
            "Si no, arrástrala por su fondo para ponerla en cualquier sitio de la pantalla."
        }
        "Through the internet" => "Por internet",
        "Same network" => "Misma red",
        "Connection to the phone and its delay" => "Conexión con el teléfono y su retardo",
        "Model and system" => "Modelo y sistema",
        "With advanced mode on the phone, open its apps in windows of their own from Screen and camera › Phone apps." => {
            "Con el modo avanzado en el teléfono, abre sus apps en ventanas propias desde Pantalla y cámara › Apps del teléfono."
        }
        "Turn the phone's screen on" => "Encender la pantalla del teléfono",
        "Turn the phone's screen off" => "Apagar la pantalla del teléfono",
        "files selected" => "archivos seleccionados",
        "Clear selection" => "Quitar la selección",
        "Search" => "Buscar",
        "Call" => "Llamar",
        "Answer" => "Contestar",
        "Decline" => "Rechazar",
        "Silence" => "Silenciar",
        "Hang up" => "Colgar",
        "Mute" => "Silenciar micrófono",
        "Speaker" => "Altavoz",
        "Hold" => "En espera",
        "Dismiss" => "Descartar",
        "Reply" => "Responder",
        "Back" => "Atrás",
        "Home" => "Inicio",
        "Recents" => "Recientes",
        "Start" => "Iniciar",
        "Stop" => "Detener",
        "No phone chosen." => "Ningún teléfono elegido.",
        "Declined" => "Rechazada",
        "Yesterday" => "Ayer",
        "Choose a phone at the top right." => "Elige un teléfono arriba a la derecha.",
        "Could not start the Broche core. See Activity." => {
            "No se pudo iniciar el núcleo de Broche. Mira Actividad."
        }
        "Is another Broche (or `broche-cli run`) already using the same data folder?" => {
            "¿Hay otro Broche (o `broche-cli run`) usando la misma carpeta de datos?"
        }
        "No paired devices. Use the Pair tab to link your phone." => {
            "No hay dispositivos vinculados. Usa la pestaña Vincular para vincular tu teléfono."
        }
        "🔔 Ring" => "🔔 Hacer sonar",
        "Stop ringing" => "Dejar de sonar",
        "Lock" => "Bloquear",
        "Sleep" => "Suspender",
        "Wake" => "Despertar",
        "Send file(s) — drop files on this window, or type a path:" => {
            "Enviar archivo(s): suéltalos en esta ventana o escribe una ruta:"
        }
        "Clipboard text:" => "Texto para el portapapeles:",
        "Send to clipboard" => "Enviar al portapapeles",
        "Open link on device:" => "Abrir enlace en el dispositivo:",
        "Unpair this device" => "Desvincular este dispositivo",
        "Phone controls" => "Controles del teléfono",
        "Volume" => "Volumen",
        "Do not disturb" => "No molestar",
        "Flashlight" => "Linterna",
        "Wi-Fi" => "Wi-Fi",
        "Bluetooth" => "Bluetooth",
        "Ringer" => "Timbre",
        "Ring" => "Sonar",
        "Vibrate" => "Vibrar",
        "Silent" => "Silencio",
        "This PC" => "Este PC",
        "PC volume" => "Volumen del PC",
        "Muted" => "Silenciado",
        "Phone is using this PC's sound (phone as speaker)" => {
            "El teléfono usa el sonido de este PC (teléfono como altavoz)"
        }
        "Let this phone control my mouse and keyboard" => {
            "Permitir que este teléfono controle mi ratón y teclado"
        }
        "The first time, your desktop asks you to approve remote control." => {
            "La primera vez, tu escritorio te pedirá aprobar el control remoto."
        }
        "Pair a new device" => "Vincular un dispositivo nuevo",
        "Show pairing code" => "Mostrar código de vinculación",
        "Waiting for a device…" => "Esperando un dispositivo…",
        "Does the other device show this code?" => "¿El otro dispositivo muestra este código?",
        "Scan this code in the Broche app:" => "Escanea este código en la app Broche:",
        "Show the phone's screen" => "Ver la pantalla del teléfono",
        "Also play the phone's sound" => "Reproducir también el sonido del teléfono",
        "The phone's own screen meanwhile" => "La pantalla del teléfono mientras tanto",
        "As usual" => "Normal",
        "Keep it on" => "Mantenerla encendida",
        "Turn it off" => "Apagarla",
        "Accept the request on the phone." => "Acepta la solicitud en el teléfono.",
        "Click and drag on the picture to touch the phone's screen." => {
            "Haz clic y arrastra sobre la imagen para tocar la pantalla del teléfono."
        }
        "Type on the phone:" => "Escribir en el teléfono:",
        "Not showing the screen." => "No se está mostrando la pantalla.",
        "Conversations" => "Conversaciones",
        "New message" => "Mensaje nuevo",
        "To (number):" => "Para (número):",
        "Message:" => "Mensaje:",
        "No conversations." => "No hay conversaciones.",
        "Recent calls" => "Llamadas recientes",
        "Dial a number" => "Marcar un número",
        "Incoming" => "Entrante",
        "Outgoing" => "Saliente",
        "Missed" => "Perdida",
        "Call in progress" => "Llamada en curso",
        "No contacts." => "No hay contactos.",
        "No notifications." => "No hay notificaciones.",
        "Albums" => "Álbumes",
        "All photos" => "Todas las fotos",
        "No photos." => "No hay fotos.",
        "Load" => "Cargar",
        "Phone" => "Teléfono",
        "Find, ring, media" => "Buscar, sonar, multimedia",
        "Capabilities" => "Capacidades",
        "Transfer started." => "Transferencia iniciada.",
        "Clipboard sent." => "Portapapeles enviado.",
        "Opened." => "Abierto.",
        "Unpaired." => "Desvinculado.",
        "Done." => "Hecho.",
        "Ringing…" => "Sonando…",
        "Stopped ringing." => "Dejó de sonar.",
        "Wake sent." => "Señal de despertar enviada.",
        "✓ Paired." => "✓ Vinculado.",
        "nothing is playing on that device" => "no hay nada reproduciéndose en ese dispositivo",
        "Dark theme" => "Tema oscuro",
        "Remote control" => "Control remoto",
        "Copied text" => "Texto copiado",
        "Copied image" => "Imagen copiada",
        "Map location" => "Ubicación en el mapa",
        "Shared document" => "Documento compartido",
        "Phone webcam" => "Webcam del teléfono",
        "on a map" => "en el mapa",
        "more" => "más",
        "Files" => "Archivos",
        "Received:" => "Recibido:",
        "Saved:" => "Guardado:",
        "Shared folder:" => "Carpeta compartida:",
        "Permissions for this phone" => "Permisos para este teléfono",
        "Wi-Fi and Bluetooth need Elevated mode on the phone." => {
            "Wi-Fi y Bluetooth requieren el modo Elevado en el teléfono."
        }
        "Your phone is looking for this PC." => "Tu teléfono está buscando este PC.",
        "Your phone wants to see this PC's screen." => "Tu teléfono quiere ver la pantalla de este PC.",
        "Allow once" => "Permitir esta vez",
        "Always allow" => "Permitir siempre",
        "Deny" => "Denegar",
        "Files you receive are saved in:" => "Los archivos que recibes se guardan en:",
        "Open folder" => "Abrir carpeta",
        "Apply" => "Aplicar",
        "Folder your phone can browse from this PC (needs the permission above):" => {
            "Carpeta de este PC que el teléfono puede explorar (requiere el permiso de arriba):"
        }
        "Phone storage" => "Almacenamiento del teléfono",
        "Turn on \"Browse the phone's storage\" in Settings › Preferences and allow it in the phone app." => {
            "Activa «Explorar el almacenamiento del teléfono» en Ajustes › Preferencias y permítelo en la app del teléfono."
        }
        "Up" => "Subir",
        "Upload a file from this PC:" => "Subir un archivo desde este PC:",
        "Upload" => "Subir",
        "New folder:" => "Carpeta nueva:",
        "Communication" => "Comunicación",
        "Files and clipboard" => "Archivos y portapapeles",
        "Screen and control" => "Pantalla y control",
        "Other permissions" => "Otros permisos",
        "You will need to pair again to reconnect." => {
            "Tendrás que volver a vincular el dispositivo para conectarte."
        }
        "Connected" => "Conectado",
        "Hide to tray" => "Ocultar en la bandeja",
        "Quit application" => "Salir de la aplicación",
        "Your desktop has no system tray host." => {
            "La bandeja requiere soporte de AppIndicator en tu escritorio."
        }
        "Choose files…" => "Elegir archivos…",
        "Choose folder…" => "Elegir carpeta…",
        "Choose files" => "Elegir archivos",
        "Choose shared folder" => "Elegir carpeta compartida",
        "Open application" => "Abrir aplicación",
        "Open received files" => "Abrir archivos recibidos",
        "Video and experiments" => "Vídeo y experimentos",
        "Stop all video" => "Detener todo el vídeo",
        "Toggle PC mute" => "Silenciar / activar audio del PC",
        "PC volume +5%" => "Volumen del PC +5 %",
        "PC volume −5%" => "Volumen del PC −5 %",
        "Use phone as speaker" => "Usar teléfono como altavoz",
        "Stop phone speaker" => "Detener altavoz del teléfono",
        "Open shared folder" => "Abrir carpeta compartida",
        "Independent modified build based on NectarLink 0.1.4 by Bee/thtbee. It is not an official NectarLink release and is not endorsed by its author." => {
            "Versión modificada independiente, basada en NectarLink 0.1.4 de Bee/thtbee. No es una versión oficial de NectarLink ni está respaldada por su autor."
        }
        "Original credits and licenses are kept." => "Se conservan los créditos y licencias originales.",
        "Create" => "Crear",
        "Theme" => "Tema",
        "Follow the system" => "Según el sistema",
        "Light (Latte)" => "Claro (Latte)",
        "Dark (Mocha)" => "Oscuro (Mocha)",
        "Messages and calls" => "Mensajes y llamadas",
        "Screen and camera" => "Pantalla y cámara",
        "Files and photos" => "Archivos y fotos",
        "Settings" => "Ajustes",
        "Calls and contacts" => "Llamadas y contactos",
        "Preferences" => "Preferencias",
        "No paired devices." => "No hay dispositivos vinculados.",
        "Link your phone" => "Vincula tu teléfono",
        "Open Broche on your phone and scan the code that appears here." => {
            "Abre Broche en tu teléfono y escanea el código que aparece aquí."
        }
        "Offline" => "Sin conexión",
        "Offline devices can't be controlled. Check both are on the same network." => {
            "Los dispositivos sin conexión no se pueden controlar. Comprueba que ambos estén en la misma red."
        }
        "Quick actions" => "Acciones rápidas",
        "Find my phone" => "Encontrar mi teléfono",
        "Show its screen" => "Ver su pantalla",
        "Use its camera" => "Usar su cámara",
        "Nothing is playing on the phone." => "No suena nada en el teléfono.",
        "Couldn't save the photo:" => "No se pudo guardar la foto:",
        "GStreamer has no H.264 encoder: install openh264 or x264 for it." => {
            "GStreamer no tiene codificador H.264: instala openh264 o x264."
        }
        "Search settings" => "Buscar en los ajustes",
        "No settings match." => "Ningún ajuste coincide.",
        "Unpair…" => "Desvincular…",
        "Send to the phone" => "Enviar al teléfono",
        "Drop files here to send them" => "Suelta archivos aquí para enviarlos",
        "They arrive in the phone's Download/Broche folder." => {
            "Llegan a la carpeta Download/Broche del teléfono."
        }
        "…or type a file path" => "…o escribe la ruta de un archivo",
        "Text for its clipboard" => "Texto para su portapapeles",
        "Send text" => "Enviar texto",
        "A link to open on the phone" => "Un enlace para abrir en el teléfono",
        "Received lately" => "Recibido hace poco",
        "Look and language" => "Apariencia e idioma",
        "This device" => "Este dispositivo",
        "About" => "Acerca de",
        "Free and open source. Your data stays on your devices." => {
            "Libre y de código abierto. Tus datos se quedan en tus dispositivos."
        }

        "Camera" => "Cámara",
        "Phone camera as webcam" => "Cámara del teléfono como webcam",
        "Back camera" => "Cámara trasera",
        "Front camera" => "Cámara frontal",
        "Start webcam" => "Iniciar webcam",
        "Stop webcam" => "Detener webcam",
        "Open the Broche notification on the phone and tap Start webcam." => {
            "Abre la notificación de Broche en el teléfono y toca «Iniciar webcam»."
        }
        "Take a photo with the phone" => "Tomar una foto con el teléfono",
        "Take photo" => "Tomar foto",
        "Scan document" => "Escanear documento",
        "Open the Broche notification on the phone to take it." => {
            "Abre la notificación de Broche en el teléfono para tomarla."
        }
        "Viewing this PC's camera on the phone is not something Broche supports." => {
            "Ver la cámara de este PC en el teléfono no es algo que Broche admita."
        }
        "Allow photos in the phone app, then refresh." => {
            "Permite las fotos en la app del teléfono y actualiza."
        }
        "Appearance" => "Apariencia",
        "Can't open this folder" => "No se puede abrir esta carpeta",
        "Choose files to send" => "Elegir archivos para enviar",
        "Choose files to upload to the phone" => "Elegir archivos para subir al teléfono",
        "Choose what this phone may do with this PC. You can change it at any time." => {
            "Elige qué puede hacer este teléfono con este PC. Puedes cambiarlo cuando quieras."
        }
        "Click a photo to download it to this PC." => "Haz clic en una foto para descargarla a este PC.",
        "Click to select, double-click to download" => "Clic para seleccionar, doble clic para descargar",
        "Close" => "Cerrar",
        "Phone apps" => "Apps del teléfono",
        "Shelf" => "Estante",
        "Island" => "Isla",
        "Keep the island after the activity" => "Mantener la isla tras la actividad",
        "At once" => "Al momento",
        "Show the phone's notifications on this PC" => "Mostrar las notificaciones del teléfono en este PC",
        "The ones that pop up on the phone also show as notifications of this PC; clicking one opens Broche. Silent and ongoing ones (music, navigation) stay on the phone." => {
            "Las que saltan en el teléfono salen también como avisos de este PC; al pulsar una se abre Broche. Las silenciosas y las continuas (música, navegación) se quedan en el teléfono."
        }
        "A pill at the top of the screen, above other windows, while there is a call, a transfer or music. It grows when the pointer is on it." => {
            "Una píldora arriba de la pantalla, encima de las demás ventanas, mientras hay una llamada, una transferencia o música. Se agranda al pasar el puntero por encima."
        }
        "Unknown caller" => "Número desconocido",
        "Incoming call" => "Llamada entrante",
        "On a call" => "En llamada",
        "Calling…" => "Llamando…",
        "Receiving" => "Recibiendo",
        "Sending" => "Enviando",
        "Drop files here to send them to" => "Suelta archivos aquí para enviarlos a",
        "Latest photo" => "Última foto",
        "Latest screenshot" => "Última captura",
        "None" => "Ninguna",
        "Latest clip" => "Último portapapeles",
        "Drag a file into another app, or open it." => "Arrastra un archivo a otra app, o ábrelo.",
        "Nothing yet." => "Nada todavía.",
        "Show in folder" => "Mostrar en la carpeta",
        "Saving the picture to Recibidos…" => "Guardando la imagen en Recibidos…",
        "Pair a phone first." => "Vincula primero un teléfono.",
        "Search apps" => "Buscar apps",
        "Open a phone app in a window of its own on this PC. It needs advanced mode on the phone." => {
            "Abre una app del teléfono en su propia ventana de este PC. Necesita el modo avanzado en el teléfono."
        }
        "Open windows" => "Ventanas abiertas",
        "Loading the phone's apps…" => "Cargando las apps del teléfono…",
        "Can't list the phone's apps:" => "No se pueden listar las apps del teléfono:",
        "No apps match." => "Ninguna app coincide.",
        "Opening the app on the phone…" => "Abriendo la app en el teléfono…",
        "The app opens in a window of its own." => "La app se abre en su propia ventana.",
        "Closing the window minimizes it and the phone stays connected. Quit from the tray icon or below." => {
            "Al cerrar la ventana se minimiza y el teléfono sigue conectado. Para salir, usa el icono de la bandeja o el botón de abajo."
        }
        "Colour scheme" => "Esquema de colores",
        "Copy" => "Copiar",
        "Dark" => "Oscuro",
        "Light" => "Claro",
        "Do not disturb on the phone" => "No molestar en el teléfono",
        "Each scheme has a light and a dark version; the mode above picks one." => {
            "Cada esquema tiene versión clara y oscura; el modo de arriba elige cuál se usa."
        }
        "Folder" => "Carpeta",
        "Folder name" => "Nombre de la carpeta",
        "Folder your phone can browse on this PC:" => "Carpeta de este PC que el teléfono puede explorar:",
        "Grid" => "Cuadrícula",
        "List" => "Lista",
        "Hidden files" => "Archivos ocultos",
        "Keep running when the window is closed" => "Seguir funcionando al cerrar la ventana",
        "Minimize" => "Minimizar",
        "Mode" => "Modo",
        "Broche opens minimized when you log in, so your phone connects right away." => {
            "Broche se abre minimizado al iniciar sesión, así el teléfono se conecta enseguida."
        }
        "New folder" => "Carpeta nueva",
        "New notifications from the phone appear here." => {
            "Aquí aparecen las notificaciones nuevas del teléfono."
        }
        "No calls." => "No hay llamadas.",
        "Or write a new message." => "O escribe un mensaje nuevo.",
        "Photo" => "Foto",
        "Pick a conversation" => "Elige una conversación",
        "Send files to the phone…" => "Enviar archivos al teléfono…",
        "Start with my session" => "Iniciar con mi sesión",
        "Name of this PC" => "Nombre de este PC",
        "How your phone shows this PC. Useful when two PCs have the same name." => {
            "Cómo ve tu teléfono este PC. Útil si dos PCs se llaman igual."
        }
        "This PC is now called" => "Este PC ahora se llama",
        "Startup and closing" => "Inicio y cierre",
        "Text size" => "Tamaño del texto",
        "The phone can only browse it if \"Browse files between these devices\" is on in Settings › Device." => {
            "El teléfono solo puede explorarla si «Explorar archivos entre estos dispositivos» está activado en Ajustes › Dispositivo."
        }
        "This folder is empty" => "Esta carpeta está vacía",
        "Turn on \"Browse files between these devices\" in Settings › Device and allow it in the phone app." => {
            "Activa «Explorar archivos entre estos dispositivos» en Ajustes › Dispositivo y permítelo en la app del teléfono."
        }
        "Upload files from this PC with the button above." => {
            "Sube archivos desde este PC con el botón de arriba."
        }
        "Upload files…" => "Subir archivos…",
        "Uploaded:" => "Subidos:",
        "Video" => "Vídeo",
        "You can also use Ctrl + and Ctrl − anywhere." => {
            "También puedes usar Ctrl + y Ctrl − en cualquier pantalla."
        }
        "Permissions" => "Permisos",
        "Previous" => "Anterior",
        "Play" => "Reproducir",
        "Pause" => "Pausa",
        "Next" => "Siguiente",
        "See and answer the phone's notifications, messages and calls here." => {
            "Ver y responder aquí las notificaciones, mensajes y llamadas del teléfono."
        }
        "Move files, photos and copied text between both devices." => {
            "Pasar archivos, fotos y texto copiado entre los dos dispositivos."
        }
        "Show screens and cameras, and let the phone use this PC's mouse and keyboard." => {
            "Mostrar pantallas y cámaras, y dejar que el teléfono use el ratón y el teclado de este PC."
        }
        "Media, quick settings and actions on this PC." => {
            "Multimedia, ajustes rápidos y acciones en este PC."
        }
        "The phone is not connected. Check that both devices are on the same network." => {
            "El teléfono no está conectado. Comprueba que ambos dispositivos estén en la misma red."
        }
        "Try again" => "Reintentar",
        "Unknown" => "Desconocido",
        "Battery" => "Batería",
        "charging" => "cargando",
        "Window" => "Ventana",
        "Keep your phone connected after you log in" => "Mantener el teléfono conectado al iniciar sesión",
        "Put in the dial box:" => "Poner en el marcador:",
        "Number ready, press Call:" => "Número listo, pulsa Llamar:",
        "Write a message" => "Escribir un mensaje",
        "Phone camera" => "Cámara del teléfono",
        "Choose the front or back camera, zoom and flash on the phone, in Broche › Webcam. Photos and scans are saved in Broche/Recibidos." => {
            "La cámara (frontal o trasera), el zoom y el flash se eligen en el teléfono, en Broche › Webcam. Las fotos y escaneos se guardan en Broche/Recibidos."
        }
        "Silent also turns on Do not disturb." => "Silencio también activa No molestar.",
        "Stop viewing its screen" => "Dejar de ver su pantalla",
        "Phone as speaker" => "Teléfono como altavoz",
        "Folders" => "Carpetas",
        "Phone files" => "Archivos del teléfono",
        "Open Broche" => "Abrir Broche",
        "Play / pause" => "Reproducir / pausa",
        "Volume up" => "Subir volumen",
        "Volume down" => "Bajar volumen",
        "Mute audio" => "Silenciar audio",
        "Lock PC" => "Bloquear PC",
        "Not connected to the Broche core." => "Sin conexión con el núcleo de Broche.",
        "Failed:" => "Error:",
        "Pairing failed:" => "Error al vincular:",
        "Paired with" => "Vinculado con",
        "Camera capture cancelled" => "Captura de cámara cancelada",
        "Transfer failed:" => "Error en la transferencia:",
        "New clipboard from the phone." => "Portapapeles nuevo del teléfono.",
        "Clipboard" => "Portapapeles",
        "Step" => "Paso",
        "of" => "de",
        "Welcome to Broche" => "Te damos la bienvenida a Broche",
        "Your Android phone and this PC, together: files, messages, notifications, its screen and camera, over your own network and end-to-end encrypted." => {
            "Tu teléfono Android y este PC, juntos: archivos, mensajes, notificaciones, su pantalla y su cámara, por tu propia red y con cifrado de extremo a extremo."
        }
        "Your phone" => "Tu teléfono",
        "already linked." => "ya está vinculado.",
        "To link another phone later, go to Settings › Pair." => {
            "Para vincular otro teléfono más adelante, ve a Ajustes › Vincular."
        }
        "1. Install Broche on your Android phone." => "1. Instala Broche en tu teléfono Android.",
        "2. Open it and choose to link a PC." => "2. Ábrelo y elige vincular un PC.",
        "3. Scan the code below and check that both show the same numbers." => {
            "3. Escanea el código de abajo y comprueba que los dos muestran los mismos números."
        }
        "Both need to be on the same Wi-Fi or network." => "Los dos tienen que estar en la misma wifi o red.",
        "What may the phone do?" => "¿Qué puede hacer el teléfono?",
        "Link a phone first. You can choose this later in Settings › Device." => {
            "Primero vincula un teléfono. Puedes elegirlo después en Ajustes › Dispositivo."
        }
        "Everyday use" => "Uso diario",
        "All set" => "Todo listo",
        "Home has what you use every day: find your phone, send files, its volume and music." => {
            "En Inicio tienes lo de cada día: encontrar el teléfono, enviar archivos, su volumen y su música."
        }
        "The tray icon keeps Broche at hand: the phone's screen, files and folders." => {
            "El icono de la bandeja tiene Broche a mano: la pantalla del teléfono, archivos y carpetas."
        }
        "Everything you chose here can be changed in Settings." => {
            "Todo lo que elegiste aquí se puede cambiar en Ajustes."
        }
        "Skip the welcome" => "Saltar la bienvenida",
        "Start using Broche" => "Empezar a usar Broche",
        "Show the welcome again" => "Ver la bienvenida otra vez",
        "Tip" => "Consejo",
        "Another tip" => "Otro consejo",
        "Hide" => "Ocultar",
        "Tips on Home" => "Consejos en Inicio",
        "A short tip about something Broche can do, a different one each time it opens." => {
            "Un consejo breve sobre algo que Broche puede hacer, distinto cada vez que se abre."
        }
        "Copy on the PC and paste on the phone: turn on Automatic clipboard in Settings › Preferences." => {
            "Copia en el PC y pega en el teléfono: activa Portapapeles automático en Ajustes › Preferencias."
        }
        "With the window closed, the tray icon still shows the phone's screen, sends files and makes it ring." => {
            "Con la ventana cerrada, el icono de la bandeja sigue mostrando la pantalla del teléfono, enviando archivos y haciéndolo sonar."
        }
        "Can't find your phone? Find my phone makes it ring, even when it's silent." => {
            "¿No encuentras el teléfono? «Encontrar mi teléfono» lo hace sonar, aunque esté en silencio."
        }
        "See the phone's camera in Screen and camera › Camera, and save a photo or a scanned document on this PC." => {
            "Mira la cámara del teléfono en Pantalla y cámara › Cámara, y guarda una foto o un documento escaneado en este PC."
        }
        "Files from the phone go to Broche › Recibidos in your home folder." => {
            "Los archivos del teléfono van a Broche › Recibidos, en tu carpeta personal."
        }
        "Drop files on this window or on the shelf to send them to the phone." => {
            "Suelta archivos en esta ventana o en el estante para enviarlos al teléfono."
        }
        "To put a picture from the phone on this PC's clipboard, share it to Copy to PC clipboard." => {
            "Para poner una imagen del teléfono en el portapapeles de este PC, compártela con «Copiar al portapapeles del PC»."
        }
        "Show the phone's notifications as desktop notifications too, in Settings › Preferences." => {
            "Muestra también las notificaciones del teléfono como avisos del escritorio, en Ajustes › Preferencias."
        }
        "The island at the top of the screen shows calls, transfers and music. Move it or turn it off in Settings › Preferences." => {
            "La isla de arriba de la pantalla muestra llamadas, transferencias y música. Muévela o apágala en Ajustes › Preferencias."
        }
        "No Wi-Fi? Connect the phone with a USB cable and USB debugging on, and Broche works through it." => {
            "¿Sin Wi-Fi? Conecta el teléfono con un cable USB y la depuración USB activada, y Broche funciona por ahí."
        }
        "If the phone doesn't connect, Settings › Device checks the connection and says why." => {
            "Si el teléfono no se conecta, Ajustes › Dispositivo revisa la conexión y te dice por qué."
        }
        "Everything the phone and this PC sent each other is in Files and photos › History." => {
            "Todo lo que se enviaron el teléfono y este PC está en Archivos y fotos › Historial."
        }
        "Reply to the phone's messages and notifications from this PC." => {
            "Responde a los mensajes y notificaciones del teléfono desde este PC."
        }
        "Press Ctrl+K to search pages, settings and actions, and run them from the keyboard." => {
            "Pulsa Ctrl+K para buscar páginas, ajustes y acciones, y usarlas desde el teclado."
        }
        "Ctrl + and Ctrl − change the text size on any page." => {
            "Ctrl + y Ctrl − cambian el tamaño del texto en cualquier página."
        }
        "Make your own PC shortcuts for the phone in Settings › Preferences." => {
            "Crea tus propios atajos del PC para el teléfono en Ajustes › Preferencias."
        }
        "Choose what the phone may do on this PC in Settings › Device." => {
            "Elige qué puede hacer el teléfono en este PC en Ajustes › Dispositivo."
        }
        "Automatic clipboard" => "Portapapeles automático",
        "Phone's sound on this PC (Bluetooth)" => "Sonido del teléfono en este PC (Bluetooth)",
        "Phone's sound back on the phone (Bluetooth)" => "Devolver el sonido al teléfono (Bluetooth)",
        "The phone plays its music and sound through this PC, as through a Bluetooth speaker. Calls are not affected." => {
            "El teléfono reproduce su música y su sonido por este PC, como por un altavoz Bluetooth. Las llamadas no cambian."
        }
        "No phone is paired with this PC over Bluetooth." => {
            "No hay ningún teléfono emparejado con este PC por Bluetooth."
        }
        "The phone didn't answer over Bluetooth: check that its Bluetooth is on." => {
            "El teléfono no respondió por Bluetooth: comprueba que tenga el Bluetooth activado."
        }
        "The phone is still connecting over Bluetooth: try again in a moment." => {
            "El teléfono aún se está conectando por Bluetooth: inténtalo de nuevo en un momento."
        }
        "This PC can't receive Bluetooth sound right now." => {
            "Este PC no puede recibir sonido por Bluetooth ahora mismo."
        }
        "The phone's sound plays on this PC." => "El sonido del teléfono suena en este PC.",
        "The phone's sound is back on the phone." => "El sonido vuelve al teléfono.",
        "The camera shows only here, in Broche: video-call apps can't use it as a camera." => {
            "La cámara solo se ve aquí, en Broche: las apps de videollamada no pueden usarla como cámara."
        }
        "What you copy on this PC goes to the phone, and a notification tells you when the phone's arrives. When off, use Send text on Home. What you copy on the phone has its own switch there: Settings › Features › Clipboard." => {
            "Lo que copies en este PC se envía al teléfono, y una notificación te avisa cuando llega lo del teléfono. Si está apagado, usa Enviar texto en Inicio. Lo que copias en el teléfono tiene su propio interruptor allí: Ajustes › Funciones › Portapapeles."
        }
        "Clipboard received." => "Portapapeles recibido.",
        "Screen sharing failed:" => "Error al compartir la pantalla:",
        "Can't run gst-launch-1.0 (GStreamer):" => "No se puede ejecutar gst-launch-1.0 (GStreamer):",
        "Can't open the screen stream:" => "No se puede abrir la transmisión de pantalla:",
        "Sharing this PC's screen" => "Compartiendo la pantalla de este PC",
        "Source code on GitHub" => "Código fuente en GitHub",
        "Stop sharing this PC's screen" => "Dejar de compartir la pantalla de este PC",
        "Can't play the ring (is ffplay or pw-play installed?):" => {
            "No se puede reproducir el timbre (¿está instalado ffplay o pw-play?):"
        }
        "Phone saved in the shared folder:" => "El teléfono guardó en la carpeta compartida:",
        "Remote control / screen sharing unavailable:" => {
            "Control remoto / compartir pantalla no disponible:"
        }
        "Remote control failed:" => "Error del control remoto:",
        "Tray:" => "Bandeja:",
        "Start with session:" => "Inicio con la sesión:",
        _ => return None,
    })
}

/// What a per-device permission allows, in one sentence (empty when the name says it all).
pub fn perm_help(name: &str) -> String {
    let es = get() == Lang::Es;
    let (en, sp) = match name {
        "notifications" => (
            "Phone notifications appear on this PC and you can answer them.",
            "Las notificaciones del teléfono aparecen en este PC y puedes responderlas.",
        ),
        "messages" => (
            "Read your SMS conversations here and send new ones.",
            "Leer aquí tus conversaciones SMS y enviar mensajes nuevos.",
        ),
        "calls" => (
            "See incoming calls, answer, hang up and dial from this PC.",
            "Ver llamadas entrantes, contestar, colgar y marcar desde este PC.",
        ),
        "contacts" => (
            "Search the phone's contacts to call or write to them.",
            "Buscar los contactos del teléfono para llamar o escribirles.",
        ),
        "files" => (
            "Send files in both directions. They are saved in Broche/Recibidos.",
            "Enviar archivos en ambos sentidos. Se guardan en Broche/Recibidos.",
        ),
        "clipboard" => (
            "Text you copy on one device can be pasted on the other, and images copied on the phone can come to this PC.",
            "El texto que copias en un dispositivo se puede pegar en el otro, y las imágenes copiadas en el teléfono pueden llegar a este PC.",
        ),
        "photos" => (
            "Browse the phone's gallery from this PC, and take a photo or scan a document with the phone.",
            "Ver la galería del teléfono desde este PC, y hacer una foto o escanear un documento con el teléfono.",
        ),
        "storage" => (
            "Explore the phone's folders from here, and the shared folder of this PC from the phone.",
            "Explorar las carpetas del teléfono desde aquí, y la carpeta compartida de este PC desde el teléfono.",
        ),
        "remote_files" => (
            "Also allow it when the devices are not on the same network (through relays).",
            "Permitirlo también cuando no estén en la misma red (a través de relés).",
        ),
        "mirroring" => (
            "Show and control the phone's screen in a window.",
            "Ver y controlar la pantalla del teléfono en una ventana.",
        ),
        "webcam" => (
            "See the phone's camera in Broche and save photos from it. Video-call apps can't use it as a camera.",
            "Ver la cámara del teléfono en Broche y guardar fotos de ella. Las apps de videollamada no pueden usarla como cámara.",
        ),
        "pc_mirror" => (
            "The phone can see this PC's screen. Your desktop asks for confirmation.",
            "El teléfono puede ver la pantalla de este PC. Tu escritorio pide confirmación.",
        ),
        "remote_input" => (
            "Use the phone as a touchpad and keyboard for this PC.",
            "Usar el teléfono como panel táctil y teclado de este PC.",
        ),
        "media" => (
            "Play, pause and skip music in either direction.",
            "Reproducir, pausar y cambiar canciones en ambos sentidos.",
        ),
        "toggles" => (
            "Change the phone's volume, ringer and Do not disturb from here.",
            "Cambiar desde aquí el volumen, el timbre y No molestar del teléfono.",
        ),
        "pc_actions" => (
            "The phone can lock or suspend this PC and change its volume.",
            "El teléfono puede bloquear o suspender este PC y cambiar su volumen.",
        ),
        "commands" => (
            "The phone can run commands you set up on this PC. Only turn it on if you need it.",
            "El teléfono puede ejecutar órdenes configuradas en este PC. Actívalo solo si lo necesitas.",
        ),
        _ => return String::new(),
    };
    if es { sp } else { en }.to_string()
}

/// The name of a per-device permission.
fn perm(name: &str, lang: Lang) -> String {
    let (en, es) = match name {
        "notifications" => ("Show its notifications", "Mostrar sus notificaciones"),
        "messages" => ("Read and send its messages", "Leer y enviar sus mensajes"),
        "calls" => ("Calls", "Llamadas"),
        "contacts" => ("Contacts", "Contactos"),
        "clipboard" => ("Share the clipboard", "Compartir el portapapeles"),
        "files" => ("Send and receive files", "Enviar y recibir archivos"),
        "media" => ("Control its media", "Controlar su multimedia"),
        "photos" => ("Browse its photos", "Ver sus fotos"),
        "toggles" => ("Change its quick settings", "Cambiar sus ajustes rápidos"),
        "pc_actions" => (
            "Let it lock or sleep this PC and change its volume",
            "Dejar que bloquee o suspenda este PC y cambie su volumen",
        ),
        "mirroring" => ("Show its screen on this PC", "Ver su pantalla en este PC"),
        "webcam" => ("Show its camera", "Ver su cámara"),
        "storage" => ("Browse files between these devices", "Explorar archivos entre estos dispositivos"),
        "pc_mirror" => ("Let it see this PC's screen", "Dejar que vea la pantalla de este PC"),
        "remote_input" => ("Let it control my mouse and keyboard", "Dejar que controle mi ratón y teclado"),
        "commands" => ("Let it run commands on this PC", "Dejar que ejecute comandos en este PC"),
        "remote_files" => ("Allow files over relays", "Permitir archivos fuera de la red local"),
        other => return other.to_string(),
    };
    if lang == Lang::Es { es } else { en }.to_string()
}

/// The core's error messages (`broche_core::Error`, `broche_protocol`) are English; this swaps
/// their fixed words inside a status line for the current language. Details that come from the
/// system or the other device (an I/O reason, a file name) stay as they are.
pub fn core_errors(line: &str) -> String {
    core_errors_in(line, get())
}

fn core_errors_in(line: &str, lang: Lang) -> String {
    if lang == Lang::En {
        return line.to_string();
    }
    // Longest first, so "network error:" isn't half-replaced by a shorter phrase.
    const ES: &[(&str, &str)] = &[
        (
            "the Local app is too old for this connection",
            "la app de este equipo es demasiado antigua para esta conexión",
        ),
        (
            "the Remote app is too old for this connection",
            "la app del otro dispositivo es demasiado antigua para esta conexión",
        ),
        ("not supported by the other device", "el otro dispositivo no lo admite"),
        ("not a Broche pairing link", "no es un enlace de vinculación de Broche"),
        ("is not supported; please update Broche", "no es compatible; actualiza Broche"),
        ("the operation was denied", "la operación fue denegada"),
        ("the operation timed out", "se agotó el tiempo de espera"),
        ("declined on this device", "rechazado en este dispositivo"),
        ("no pairing in progress", "no hay ninguna vinculación en curso"),
        ("device is not paired", "el dispositivo no está vinculado"),
        ("device is offline", "el dispositivo no está conectado"),
        ("it no longer exists", "ya no existe"),
        ("too large to send", "demasiado grande para enviarlo"),
        ("invalid pairing link:", "enlace de vinculación no válido:"),
        ("pairing link version", "la versión del enlace de vinculación"),
        ("pairing link is missing", "al enlace de vinculación le falta"),
        ("pairing link has an invalid", "el enlace de vinculación tiene un valor no válido:"),
        ("peer returned error", "el otro dispositivo devolvió el error"),
        ("protocol error:", "error de protocolo:"),
        ("network error:", "error de red:"),
        ("storage error:", "error de almacenamiento:"),
        ("internal error:", "error interno:"),
        ("i/o error:", "error de E/S:"),
        ("stream closed", "la conexión se cerró"),
    ];
    let mut out = line.to_string();
    for (en, es) in ES {
        out = out.replace(en, es);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Lang, core_errors_in, es};

    #[test]
    fn every_program_feature_has_a_spanish_name() {
        let missing: Vec<&str> =
            crate::deps::NEEDS.iter().map(|n| n.feature).filter(|f| es(f).is_none()).collect();
        assert!(missing.is_empty(), "no Spanish for {missing:?}");
    }

    #[test]
    fn core_errors_read_in_spanish() {
        assert_eq!(
            core_errors_in("Error: device is offline", Lang::Es),
            "Error: el dispositivo no está conectado"
        );
        assert_eq!(
            core_errors_in("Failed: network error: connection lost", Lang::Es),
            "Failed: error de red: connection lost",
            "only the core's own words change; the detail stays"
        );
        assert_eq!(core_errors_in("device is offline", Lang::En), "device is offline");
    }

    /// Every literal passed to `t("…")` in the app has a Spanish translation,
    /// so the interface is complete in both languages.
    #[test]
    fn every_tip_has_a_spanish_translation() {
        let missing: Vec<_> = crate::shell::TIPS.iter().filter(|k| super::es(k).is_none()).collect();
        assert!(missing.is_empty(), "tips without Spanish: {missing:?}");
    }

    #[test]
    fn every_text_has_a_spanish_translation() {
        let sources = [
            include_str!("main.rs"),
            include_str!("shell.rs"),
            include_str!("tray.rs"),
            include_str!("platform.rs"),
            include_str!("mirror.rs"),
            include_str!("pc_media.rs"),
            include_str!("kit.rs"),
            include_str!("settings.rs"),
            include_str!("resources.rs"),
            include_str!("theme.rs"),
            include_str!("welcome.rs"),
            include_str!("history.rs"),
            include_str!("doctor.rs"),
            include_str!("palette.rs"),
            include_str!("apps.rs"),
            include_str!("shelf.rs"),
            include_str!("island.rs"),
            include_str!("phone_notify.rs"),
            include_str!("bt_music.rs"),
            include_str!("bt_calls.rs"),
            include_str!("deck.rs"),
            include_str!("clipwatch.rs"),
            include_str!("laser.rs"),
            include_str!("xdnd.rs"),
        ];
        let mut missing = Vec::new();
        for src in sources {
            let mut rest = src;
            while let Some(i) = rest.find("t(\"") {
                // Only the `t` function, not e.g. `set("` or `start("`.
                let is_call = i == 0
                    || !rest.as_bytes()[i - 1].is_ascii_alphanumeric() && rest.as_bytes()[i - 1] != b'_';
                rest = &rest[i + 3..];
                // The literal ends at the first quote not escaped with `\\`.
                let mut key = String::new();
                let mut chars = rest.char_indices();
                let mut end = rest.len();
                while let Some((j, c)) = chars.next() {
                    match c {
                        '\\' => {
                            if let Some((_, e)) = chars.next() {
                                key.push(match e {
                                    'n' => '\n',
                                    other => other,
                                });
                            }
                        }
                        '"' => {
                            end = j;
                            break;
                        }
                        c => key.push(c),
                    }
                }
                rest = &rest[end..];
                if is_call && !key.starts_with("perm:") && super::es(&key).is_none() {
                    missing.push(key);
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "sin traducción al español: {missing:#?}");
    }
}
