// SPDX-License-Identifier: GPL-3.0-or-later
//! PipeWire HFP control on the session bus, with network fallback when no call matches.
use futures_util::StreamExt;
use std::sync::{Arc, OnceLock, RwLock};
use zbus::{Connection, Proxy, zvariant::OwnedValue};

const SERVICE: &str = "org.pipewire.Telephony";
const ROOT: &str = "/org/pipewire/Telephony";
const AG: &str = "org.pipewire.Telephony.AudioGateway1";
const CALL: &str = "org.pipewire.Telephony.Call1";
const TRANSPORT: &str = "org.pipewire.Telephony.AudioGatewayTransport1";

#[derive(Clone, Debug, Default)]
pub struct Call {
    pub path: String,
    #[allow(dead_code)] // read by the tests
    pub state: String,
    pub number: String,
}
#[derive(Clone, Debug, Default)]
pub struct Gateway {
    pub path: String,
    pub address: String,
    pub alias: String,
    pub transport_state: String,
    pub codec: u8,
    pub calls: Vec<Call>,
}

pub fn select_gateway<'a>(phones: &'a [Gateway], preferred: &str) -> Option<&'a Gateway> {
    if phones.len() == 1 {
        return phones.first();
    }
    let mut matches =
        phones.iter().filter(|p| !preferred.is_empty() && p.address.eq_ignore_ascii_case(preferred));
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}
pub fn select_call<'a>(calls: &'a [Call], number: &str) -> Option<&'a Call> {
    let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
    let number = digits(number);
    if let [only] = calls {
        // A lone call is taken unless both sides know a number and they differ (another phone's call).
        let known = digits(&only.number);
        return (number.is_empty() || known.is_empty() || known == number).then_some(only);
    }
    if number.is_empty() {
        return None;
    }
    let mut matches = calls.iter().filter(|c| digits(&c.number) == number);
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

fn route<'a>(
    phones: &'a [Gateway],
    preferred: &str,
    number: &str,
    cmd: broche_core::CallCommand,
) -> Option<&'a str> {
    use broche_core::CallCommand;
    if !matches!(cmd, CallCommand::Answer | CallCommand::Decline) {
        return None;
    }
    let phone = select_gateway(phones, preferred)?;
    select_call(&phone.calls, number).map(|c| c.path.as_str())
}

fn check_permission(node: &broche_core::Node, peer: broche_core::DeviceId) -> Result<(), String> {
    let toggles = node.device_toggles(peer).map_err(|e| e.to_string())?;
    if !toggles.iter().any(|(name, enabled)| *name == "calls" && *enabled) {
        return Err(broche_core::Error::Denied.to_string());
    }
    Ok(())
}

pub async fn dispatch(
    node: &broche_core::Node,
    peer: broche_core::DeviceId,
    call: &broche_core::CallState,
    cmd: broche_core::CallCommand,
) -> Result<(), String> {
    check_permission(node, peer)?;
    let settings = crate::settings::Settings::load();
    let preferred = settings.bt_phone;
    let target =
        route(&current().read().unwrap(), &preferred, call.number.as_deref().unwrap_or_default(), cmd)
            .map(str::to_owned);
    if let Some(path) = target {
        let conn = Connection::session().await.map_err(|e| e.to_string())?;
        // Do not retry over the network: a Bluetooth error may follow a successful action.
        if cmd == broche_core::CallCommand::Answer {
            let (ag, _) = path.rsplit_once('/').ok_or("Invalid Bluetooth call path")?;
            answer_on_pc(&conn, &path, ag, settings.bt_talk_on_answer).await.map_err(|e| e.to_string())
        } else {
            control(&conn, &path, cmd).await.map_err(|e| e.to_string())
        }
    } else {
        node.call_command(peer, call.id.clone(), cmd).await.map_err(|e| e.to_string())
    }
}

async fn answer_on_pc(conn: &Connection, call: &str, ag: &str, automatic: bool) -> zbus::Result<()> {
    answer(conn, call).await?;
    // The call is already answered: if the PC audio can't start, say nothing failed. "Talk on this PC" stays on screen.
    if automatic && let Err(e) = talk_on_pc(conn, ag).await {
        eprintln!("Bluetooth call audio did not move to this PC: {e}");
    }
    Ok(())
}

/// The phone chosen in Preferences, kept in memory: `audio_gateway` runs every frame during a call.
fn preferred() -> &'static RwLock<String> {
    static PREFERRED: OnceLock<RwLock<String>> = OnceLock::new();
    PREFERRED.get_or_init(|| RwLock::new(crate::settings::Settings::load().bt_phone))
}
/// The address of the phone chosen for calls (empty when none was chosen).
pub fn preferred_phone() -> String {
    preferred().read().unwrap().clone()
}
pub fn set_preferred(address: &str) {
    address.clone_into(&mut preferred().write().unwrap());
}

pub fn audio_gateway(number: &str) -> Option<Gateway> {
    let preferred = preferred_phone();
    let phones = current().read().unwrap();
    let ag = select_gateway(&phones, &preferred)?;
    select_call(&ag.calls, number)?;
    Some(ag.clone())
}

pub async fn activate_audio(
    node: &broche_core::Node,
    peer: broche_core::DeviceId,
    ag: &str,
) -> Result<(), String> {
    check_permission(node, peer)?;
    let conn = Connection::session().await.map_err(|e| e.to_string())?;
    talk_on_pc(&conn, ag).await.map_err(|e| e.to_string())
}

async fn control(conn: &Connection, path: &str, cmd: broche_core::CallCommand) -> zbus::Result<()> {
    match cmd {
        broche_core::CallCommand::Answer => answer(conn, path).await,
        broche_core::CallCommand::Decline => hangup(conn, path).await,
        _ => Err(zbus::Error::Failure("Unsupported Bluetooth call command".into())),
    }
}

async fn objects(conn: &Connection, path: &str) -> zbus::Result<zbus::fdo::ManagedObjects> {
    Proxy::new(conn, SERVICE, path, "org.freedesktop.DBus.ObjectManager")
        .await?
        .call("GetManagedObjects", &())
        .await
}
pub(crate) fn text(props: &std::collections::HashMap<String, OwnedValue>, key: &str) -> String {
    props.get(key).and_then(|v| <&str>::try_from(v).ok()).unwrap_or_default().into()
}
pub async fn snapshot(conn: &Connection) -> zbus::Result<Vec<Gateway>> {
    let mut phones = Vec::new();
    for (path, interfaces) in objects(conn, ROOT).await? {
        let Some(props) = interfaces.get(AG) else { continue };
        let mut phone =
            Gateway { path: path.to_string(), address: text(props, "Address"), ..Default::default() };
        if let Some(t) = interfaces.get(TRANSPORT) {
            phone.transport_state = text(t, "State");
            phone.codec = t.get("Codec").and_then(|v| u8::try_from(v).ok()).unwrap_or_default();
        }
        // Gateways can disappear between the two ObjectManager requests.
        let Ok(calls) = objects(conn, &phone.path).await else { continue };
        for (path, interfaces) in calls {
            if let Some(c) = interfaces.get(CALL) {
                phone.calls.push(Call {
                    path: path.to_string(),
                    state: text(c, "State"),
                    number: text(c, "LineIdentification"),
                });
            }
        }
        phone.calls.sort_by(|a, b| a.path.cmp(&b.path));
        phones.push(phone);
    }
    phones.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(phones)
}

async fn phone_alias(conn: &Connection, address: &str) -> Option<String> {
    let proxy = Proxy::new(conn, "org.bluez", "/", "org.freedesktop.DBus.ObjectManager").await.ok()?;
    let objects: zbus::fdo::ManagedObjects = proxy.call("GetManagedObjects", &()).await.ok()?;
    objects
        .values()
        .filter_map(|interfaces| interfaces.get("org.bluez.Device1"))
        .find(|p| text(p, "Address").eq_ignore_ascii_case(address))
        .map(|p| text(p, "Alias"))
}

pub fn current() -> &'static Arc<RwLock<Vec<Gateway>>> {
    static STATE: OnceLock<Arc<RwLock<Vec<Gateway>>>> = OnceLock::new();
    STATE.get_or_init(|| Arc::new(RwLock::new(Vec::new())))
}

static AVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn available() -> bool {
    AVAILABLE.load(std::sync::atomic::Ordering::Relaxed)
}

async fn watch_connection(
    conn: &Connection,
    state: &RwLock<Vec<Gateway>>,
    bluez: Option<&Connection>,
) -> zbus::Result<()> {
    // Subscribe before enumerating; this covers both nested managers and property changes.
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(SERVICE)?
        .path_namespace(ROOT)?
        .build();
    let mut signals = zbus::MessageStream::for_match_rule(rule, conn, Some(64)).await?;
    let mut retry = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        let result = snapshot(conn).await;
        AVAILABLE.store(result.is_ok(), std::sync::atomic::Ordering::Relaxed);
        let mut phones = result.unwrap_or_default();
        if let Some(bluez) = bluez {
            for phone in &mut phones {
                phone.alias = phone_alias(bluez, &phone.address).await.unwrap_or_default();
            }
        }
        *state.write().unwrap() = phones;
        tokio::select! {
            signal = signals.next() => { signal.ok_or(zbus::Error::Failure("Telephony signal stream ended".into()))??; }
            _ = retry.tick() => {}
        }
    }
}
pub async fn watch() {
    loop {
        if let Ok(conn) = Connection::session().await {
            let bluez = Connection::system().await.ok();
            let _ = watch_connection(&conn, current(), bluez.as_ref()).await;
        }
        current().write().unwrap().clear();
        AVAILABLE.store(false, std::sync::atomic::Ordering::Relaxed);
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}
async fn command(conn: &Connection, path: &str, interface: &str, method: &str) -> zbus::Result<()> {
    Proxy::new(conn, SERVICE, path, interface).await?.call(method, &()).await
}
async fn call_state(conn: &Connection, call: &str) -> zbus::Result<String> {
    let proxy: Proxy<'_> = zbus::proxy::Builder::new(conn)
        .destination(SERVICE)?
        .path(call)?
        .interface(CALL)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    proxy.get_property("State").await
}

pub async fn answer(conn: &Connection, call: &str) -> zbus::Result<()> {
    if call_state(conn, call).await? == "active" {
        return Ok(());
    }
    let result = command(conn, call, CALL, "Answer").await;
    // The phone can answer between the state read and our command.
    if matches!(&result, Err(zbus::Error::MethodError(name, ..)) if name.as_str() == "org.pipewire.Telephony.Error.InvalidState")
        && call_state(conn, call).await.is_ok_and(|state| state == "active")
    {
        return Ok(());
    }
    result
}
pub async fn hangup(conn: &Connection, call: &str) -> zbus::Result<()> {
    command(conn, call, CALL, "Hangup").await
}
pub async fn talk_on_pc(conn: &Connection, ag: &str) -> zbus::Result<()> {
    let proxy = Proxy::new(conn, SERVICE, ag, TRANSPORT).await?;
    if proxy.get_property::<String>("State").await? != "active" {
        proxy.call::<_, _, ()>("Activate", &()).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_only_answer_and_hangup_to_a_matching_bluetooth_call() {
        use broche_core::CallCommand::*;
        let phone = Gateway {
            calls: vec![Call { path: "/ag1/call1".into(), number: "555123".into(), ..Default::default() }],
            ..Default::default()
        };
        assert!(route(&[], "", "555123", Answer).is_none());
        for cmd in [Answer, Decline] {
            assert_eq!(route(std::slice::from_ref(&phone), "", "555123", cmd).unwrap(), "/ag1/call1");
        }
        for cmd in [Silence, Mute(true), Speaker(true), Hold(true), Dtmf('1'), Volume(true)] {
            assert!(route(std::slice::from_ref(&phone), "", "555123", cmd).is_none());
        }
        assert!(route(&[phone.clone(), phone], "", "555123", Answer).is_none());
    }

    #[test]
    fn selects_only_an_unambiguous_phone_and_call() {
        let a = Gateway { path: "/ag1".into(), address: "00:11:22:33:44:55".into(), ..Default::default() };
        let b = Gateway { path: "/ag2".into(), address: "00:11:22:33:44:66".into(), ..Default::default() };
        assert!(select_gateway(&[], "").is_none());
        assert_eq!(select_gateway(std::slice::from_ref(&a), "old").unwrap().path, a.path);
        assert!(select_gateway(&[a.clone(), b.clone()], "").is_none());
        assert_eq!(select_gateway(&[a.clone(), b.clone()], &b.address).unwrap().path, b.path);
        assert!(select_gateway(&[a, b], "old").is_none());
        let a = Call { path: "/ag1/call1".into(), number: "+1 (555) 123".into(), ..Default::default() };
        let b = Call { path: "/ag1/call2".into(), number: "999".into(), ..Default::default() };
        assert_eq!(select_call(std::slice::from_ref(&a), "").unwrap().path, a.path);
        assert_eq!(select_call(&[a.clone(), b.clone()], "1-555-123").unwrap().path, a.path);
        assert!(select_call(&[a.clone(), b.clone()], "").is_none());
        assert!(select_call(&[a.clone(), a], "1555123").is_none());
        assert!(select_call(std::slice::from_ref(&b), "").is_some());
        assert!(select_call(std::slice::from_ref(&b), "123").is_none());
    }
}

#[cfg(test)]
mod bus_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    async fn wait_for(state: &RwLock<Vec<Gateway>>, condition: impl Fn(&[Gateway]) -> bool) {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !condition(&state.read().unwrap()) {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    struct Phone;
    #[zbus::interface(name = "org.pipewire.Telephony.AudioGateway1")]
    impl Phone {
        #[zbus(property)]
        fn address(&self) -> &str {
            "00:11:22:33:44:55"
        }
    }
    struct Transport(Arc<AtomicBool>);
    #[zbus::interface(name = "org.pipewire.Telephony.AudioGatewayTransport1")]
    impl Transport {
        fn activate(&self) -> zbus::fdo::Result<()> {
            if self.0.swap(true, Ordering::SeqCst) {
                return Err(zbus::fdo::Error::Failed("Already active".into()));
            }
            Ok(())
        }
        #[zbus(property)]
        fn state(&self) -> &str {
            if self.0.load(Ordering::SeqCst) { "active" } else { "idle" }
        }
    }
    #[derive(Debug, zbus::DBusError)]
    #[zbus(prefix = "org.pipewire.Telephony.Error")]
    enum CallError {
        InvalidState(String),
    }
    struct Incoming {
        answered: Arc<AtomicBool>,
        answer_during_state_read: Arc<AtomicBool>,
    }
    #[zbus::interface(name = "org.pipewire.Telephony.Call1")]
    impl Incoming {
        fn answer(&self) -> Result<(), CallError> {
            if self.answered.swap(true, Ordering::SeqCst) {
                return Err(CallError::InvalidState(
                    "The current state does not allow this method call".into(),
                ));
            }
            Ok(())
        }
        fn hangup(&self) {
            self.answered.store(false, Ordering::SeqCst);
        }
        #[zbus(property)]
        fn state(&self) -> &str {
            let active = self.answered.load(Ordering::SeqCst);
            if self.answer_during_state_read.swap(false, Ordering::SeqCst) {
                self.answered.store(true, Ordering::SeqCst);
            }
            if active { "active" } else { "incoming" }
        }
        #[zbus(property)]
        fn line_identification(&self) -> &str {
            "555123"
        }
    }
    #[tokio::test]
    #[ignore = "Run under dbus-run-session: owns the telephony service name"]
    async fn discovers_answers_and_removes_call() {
        let answered = Arc::new(AtomicBool::new(false));
        let answer_during_state_read = Arc::new(AtomicBool::new(false));
        let audio = Arc::new(AtomicBool::new(false));
        let ag = format!("{ROOT}/ag1");
        let call = format!("{ag}/call1");
        let server = zbus::connection::Builder::session()
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .serve_at(ROOT, zbus::fdo::ObjectManager)
            .unwrap()
            .serve_at(ag.as_str(), zbus::fdo::ObjectManager)
            .unwrap()
            .serve_at(ag.as_str(), Phone)
            .unwrap()
            .serve_at(ag.as_str(), Transport(audio.clone()))
            .unwrap()
            .serve_at(
                call.as_str(),
                Incoming {
                    answered: answered.clone(),
                    answer_during_state_read: answer_during_state_read.clone(),
                },
            )
            .unwrap()
            .build()
            .await
            .unwrap();
        let conn = Connection::session().await.unwrap();
        let observed = Arc::new(RwLock::new(Vec::new()));
        let watch_state = observed.clone();
        let watch_conn = conn.clone();
        let watcher = tokio::spawn(async move { watch_connection(&watch_conn, &watch_state, None).await });
        wait_for(&observed, |p| p.len() == 1 && p[0].calls.len() == 1).await;
        let state = snapshot(&conn).await.unwrap();
        assert_eq!(state.len(), 1);
        assert_eq!(state[0].calls[0].state, "incoming");
        let path = route(&state, "", "555123", broche_core::CallCommand::Answer).unwrap();
        control(&conn, path, broche_core::CallCommand::Answer).await.unwrap();
        assert!(answered.load(Ordering::SeqCst));
        answer_on_pc(&conn, &call, &ag, true).await.unwrap();
        assert!(audio.load(Ordering::SeqCst));
        talk_on_pc(&conn, &ag).await.unwrap();
        audio.store(false, Ordering::SeqCst);
        answer_on_pc(&conn, &call, &ag, false).await.unwrap();
        assert!(!audio.load(Ordering::SeqCst));
        answered.store(false, Ordering::SeqCst);
        answer_during_state_read.store(true, Ordering::SeqCst);
        answer_on_pc(&conn, &call, &ag, false).await.unwrap();
        assert!(answered.load(Ordering::SeqCst));
        assert_eq!(snapshot(&conn).await.unwrap()[0].calls[0].state, "active");
        control(&conn, path, broche_core::CallCommand::Decline).await.unwrap();
        assert!(!answered.load(Ordering::SeqCst));
        server.object_server().remove::<Incoming, _>(call.as_str()).await.unwrap();
        assert!(control(&conn, path, broche_core::CallCommand::Answer).await.is_err());
        wait_for(&observed, |p| p.len() == 1 && p[0].calls.is_empty()).await;
        server.object_server().remove::<Phone, _>(ag.as_str()).await.unwrap();
        wait_for(&observed, |p| p.is_empty()).await;
        watcher.abort();
    }
}
