use std::{borrow::Cow, str::FromStr, sync::Arc};

use heck::ToTitleCase;
use openscq30_lib::{
    DeviceModel, OpenSCQ30Session,
    connection::{ConnectionDescriptor, ConnectionStatus},
    device::OpenSCQ30Device,
    settings::{CategoryId, Select, Setting, SettingId, Value},
    storage::PairedDevice,
};
use qmetaobject::*;

use crate::tray::{TrayCommand, TrayHandle};

/// The single device this personal build targets.
const DEVICE_NAME: &str = "Soundcore R50i NC";

/// Result of a background connect attempt, marshalled back onto the Qt thread.
enum ConnectEvent {
    Connected(Arc<dyn OpenSCQ30Device + Send + Sync>),
    Failed(String),
    NoDevices,
}

#[derive(QObject)]
pub struct Backend {
    base: qt_base_class!(trait QObject),

    // Runtime plumbing (not exposed to QML).
    runtime: tokio::runtime::Handle,
    session: Arc<OpenSCQ30Session>,
    current_device: Option<Arc<dyn OpenSCQ30Device + Send + Sync>>,
    available_descriptors: Vec<ConnectionDescriptor>,
    tray: TrayHandle,

    // Connection state.
    state: qt_property!(String; NOTIFY state_changed),
    state_changed: qt_signal!(),
    device_name: qt_property!(String; NOTIFY device_name_changed),
    device_name_changed: qt_signal!(),
    status_message: qt_property!(String; NOTIFY status_message_changed),
    status_message_changed: qt_signal!(),

    // Battery.
    battery_left: qt_property!(String; NOTIFY battery_changed),
    battery_right: qt_property!(String; NOTIFY battery_changed),
    charging_left: qt_property!(bool; NOTIFY battery_changed),
    charging_right: qt_property!(bool; NOTIFY battery_changed),
    battery_changed: qt_signal!(),

    // Headline ANC control.
    anc_mode: qt_property!(String; NOTIFY anc_mode_changed),
    anc_mode_changed: qt_signal!(),

    // Pairing list.
    available_devices: qt_property!(QVariantList; NOTIFY available_devices_changed),
    available_devices_changed: qt_signal!(),

    // Settings browsing.
    categories: qt_property!(QVariantList; NOTIFY categories_changed),
    categories_changed: qt_signal!(),
    current_category: qt_property!(String; NOTIFY current_category_changed),
    current_category_changed: qt_signal!(),
    settings: qt_property!(QVariantList; NOTIFY settings_changed),
    settings_changed: qt_signal!(),

    busy: qt_property!(bool; NOTIFY busy_changed),
    busy_changed: qt_signal!(),

    // QML-invokable methods.
    startup: qt_method!(fn(&mut self)),
    list_devices: qt_method!(fn(&mut self)),
    pair_and_connect: qt_method!(fn(&mut self, mac: String, demo: bool)),
    disconnect: qt_method!(fn(&mut self)),
    set_category: qt_method!(fn(&mut self, id: String)),
    set_toggle: qt_method!(fn(&mut self, id: String, value: bool)),
    set_select: qt_method!(fn(&mut self, id: String, value: String)),
    set_range: qt_method!(fn(&mut self, id: String, value: i32)),
    set_equalizer_band: qt_method!(fn(&mut self, id: String, index: i32, value: i32)),
    trigger_action: qt_method!(fn(&mut self, id: String)),
    set_anc_mode: qt_method!(fn(&mut self, mode: String)),
    quit: qt_method!(fn(&mut self)),

    // Tray signals.
    open_requested: qt_signal!(),
}

impl Backend {
    pub fn new(
        session: Arc<OpenSCQ30Session>,
        runtime: tokio::runtime::Handle,
        tray: TrayHandle,
    ) -> Self {
        Self {
            base: Default::default(),
            runtime,
            session,
            current_device: None,
            available_descriptors: Vec::new(),
            tray,
            state: "disconnected".to_string(),
            state_changed: Default::default(),
            device_name: String::new(),
            device_name_changed: Default::default(),
            status_message: "Not connected".to_string(),
            status_message_changed: Default::default(),
            battery_left: String::new(),
            battery_right: String::new(),
            charging_left: false,
            charging_right: false,
            battery_changed: Default::default(),
            anc_mode: String::new(),
            anc_mode_changed: Default::default(),
            available_devices: QVariantList::default(),
            available_devices_changed: Default::default(),
            categories: QVariantList::default(),
            categories_changed: Default::default(),
            current_category: String::new(),
            current_category_changed: Default::default(),
            settings: QVariantList::default(),
            settings_changed: Default::default(),
            busy: false,
            busy_changed: Default::default(),
            startup: Default::default(),
            list_devices: Default::default(),
            pair_and_connect: Default::default(),
            disconnect: Default::default(),
            set_category: Default::default(),
            set_toggle: Default::default(),
            set_select: Default::default(),
            set_range: Default::default(),
            set_equalizer_band: Default::default(),
            trigger_action: Default::default(),
            set_anc_mode: Default::default(),
            quit: Default::default(),
            open_requested: Default::default(),
        }
    }

    // --- synchronous state helpers (always run on the Qt thread) ---

    fn set_state(&mut self, state: &str) {
        if self.state != state {
            self.state = state.to_string();
            self.state_changed();
        }
    }

    fn set_busy(&mut self, busy: bool) {
        if self.busy != busy {
            self.busy = busy;
            self.busy_changed();
        }
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
        self.status_message_changed();
    }

    fn set_connected(&mut self, device: Arc<dyn OpenSCQ30Device + Send + Sync>) {
        self.current_device = Some(device.clone());
        self.device_name = DEVICE_NAME.to_string();
        self.device_name_changed();
        self.set_state("connected");
        self.set_busy(false);
        self.tray
            .update(|tray| tray.set_device(Some(device.clone())));
        self.refresh_all();
        self.spawn_watchers(device);
    }

    fn handle_connect_event(&mut self, event: ConnectEvent) {
        match event {
            ConnectEvent::Connected(device) => self.set_connected(device),
            ConnectEvent::Failed(message) => {
                self.set_busy(false);
                self.set_state("disconnected");
                self.set_status(message);
            }
            ConnectEvent::NoDevices => {
                self.set_busy(false);
                self.set_state("disconnected");
                self.set_status("No paired device yet");
            }
        }
    }

    fn handle_disconnect(&mut self) {
        if self.current_device.is_some() {
            self.current_device = None;
            self.device_name = String::new();
            self.device_name_changed();
            self.anc_mode = String::new();
            self.anc_mode_changed();
            self.set_state("disconnected");
            self.set_status("Device disconnected");
            self.tray.update(|tray| tray.set_device(None));
            self.refresh_all();
        }
    }

    /// Re-reads every property that derives from the connected device.
    fn refresh_all(&mut self) {
        let Some(device) = self.current_device.clone() else {
            self.categories = QVariantList::default();
            self.categories_changed();
            self.current_category = String::new();
            self.current_category_changed();
            self.settings = QVariantList::default();
            self.settings_changed();
            self.battery_left = String::new();
            self.battery_right = String::new();
            self.charging_left = false;
            self.charging_right = false;
            self.battery_changed();
            self.anc_mode = String::new();
            self.anc_mode_changed();
            return;
        };

        self.battery_left =
            information_value(device.as_ref(), SettingId::BatteryLevelLeft).unwrap_or_default();
        self.battery_right =
            information_value(device.as_ref(), SettingId::BatteryLevelRight).unwrap_or_default();
        self.charging_left = information_value(device.as_ref(), SettingId::IsChargingLeft)
            .is_some_and(|value| value.eq_ignore_ascii_case("yes"));
        self.charging_right = information_value(device.as_ref(), SettingId::IsChargingRight)
            .is_some_and(|value| value.eq_ignore_ascii_case("yes"));
        self.battery_changed();

        self.anc_mode =
            current_select_value(device.as_ref(), SettingId::AmbientSoundMode).unwrap_or_default();
        self.anc_mode_changed();

        self.rebuild_categories(device.as_ref());
        self.rebuild_settings(device.as_ref());

        // Keep the tray tooltip/menu in sync (battery, ANC mode).
        self.tray.update(|_| {});
    }

    fn rebuild_categories(&mut self, device: &dyn OpenSCQ30Device) {
        let categories = device.categories();
        if categories.is_empty() {
            self.categories = QVariantList::default();
            self.categories_changed();
            return;
        }

        let mut list = QVariantList::default();
        for category in &categories {
            let mut item = QVariantMap::default();
            put_str(&mut item, "id", &category.to_string());
            put_str(&mut item, "label", &label_for_category(*category));
            list.push(QVariant::from(item));
        }
        self.categories = list;
        self.categories_changed();

        let valid = categories
            .iter()
            .any(|category| category.to_string() == self.current_category);
        if !valid {
            self.current_category = categories[0].to_string();
            self.current_category_changed();
        }
    }

    fn rebuild_settings(&mut self, device: &dyn OpenSCQ30Device) {
        let Some(category) = device
            .categories()
            .into_iter()
            .find(|category| category.to_string() == self.current_category)
        else {
            self.settings = QVariantList::default();
            self.settings_changed();
            return;
        };

        let mut list = QVariantList::default();
        for setting_id in device.settings_in_category(&category) {
            if let Some(setting) = device.setting(&setting_id) {
                list.push(QVariant::from(setting_to_map(setting_id, &setting)));
            }
        }
        self.settings = list;
        self.settings_changed();
    }

    fn spawn_watchers(&self, device: Arc<dyn OpenSCQ30Device + Send + Sync>) {
        let runtime = self.runtime.clone();
        let qptr = QPointer::from(self);
        let disc_qptr = qptr.clone();

        // Settings changes -> refresh the QML-facing snapshot.
        let refresh_cb = queued_callback(move |()| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow_mut().refresh_all();
            }
        });
        let mut watch = device.watch_for_changes();
        runtime.spawn(async move {
            while watch.changed().await.is_ok() {
                refresh_cb(());
            }
        });

        // Connection loss -> return to the disconnected screen.
        let disc_cb = queued_callback(move |()| {
            if let Some(pinned) = disc_qptr.as_pinned() {
                pinned.borrow_mut().handle_disconnect();
            }
        });
        let mut status = device.connection_status();
        runtime.spawn(async move {
            loop {
                if matches!(*status.borrow(), ConnectionStatus::Disconnected) {
                    disc_cb(());
                    break;
                }
                if status.changed().await.is_err() {
                    break;
                }
            }
        });
    }

    fn send_setting(
        &mut self,
        device: Arc<dyn OpenSCQ30Device + Send + Sync>,
        values: Vec<(SettingId, Value)>,
    ) {
        self.set_busy(true);
        let runtime = self.runtime.clone();
        let qptr = QPointer::from(&*self);
        let done = queued_callback(move |result: Result<(), String>| {
            if let Some(pinned) = qptr.as_pinned() {
                let mut this = pinned.borrow_mut();
                this.set_busy(false);
                if let Err(err) = result {
                    this.set_status(err);
                }
                this.refresh_all();
            }
        });
        runtime.spawn(async move {
            let result = device
                .set_setting_values(values)
                .await
                .map_err(|err| format!("{err:#}"));
            done(result);
        });
    }
}

impl Backend {
    fn startup(&mut self) {
        if self.current_device.is_some() {
            return;
        }
        self.set_busy(true);
        self.set_state("connecting");
        self.set_status("Looking for your device…");

        let session = self.session.clone();
        let runtime = self.runtime.clone();
        let qptr = QPointer::from(&*self);
        let done = queued_callback(move |event: ConnectEvent| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow_mut().handle_connect_event(event);
            }
        });
        runtime.spawn(async move {
            let event = match session.paired_devices().await {
                Ok(devices) => {
                    let target = devices
                        .iter()
                        .find(|device| device.model == DeviceModel::SoundcoreA3959)
                        .or_else(|| devices.first())
                        .copied();
                    match target {
                        Some(paired) => match session.connect(paired.mac_address).await {
                            Ok(device) => ConnectEvent::Connected(device),
                            Err(err) => ConnectEvent::Failed(format!("{err:#}")),
                        },
                        None => ConnectEvent::NoDevices,
                    }
                }
                Err(err) => ConnectEvent::Failed(format!("{err:#}")),
            };
            done(event);
        });
    }

    fn list_devices(&mut self) {
        self.set_busy(true);
        self.set_status("Scanning for devices…");

        let session = self.session.clone();
        let runtime = self.runtime.clone();
        let qptr = QPointer::from(&*self);
        let done = queued_callback(move |result: Result<Vec<ConnectionDescriptor>, String>| {
            if let Some(pinned) = qptr.as_pinned() {
                let mut this = pinned.borrow_mut();
                this.set_busy(false);
                match result {
                    Ok(devices) => {
                        this.available_descriptors = devices;
                        this.refresh_available_devices();
                        this.set_status("Select a device to pair");
                    }
                    Err(err) => {
                        this.available_descriptors = Vec::new();
                        this.refresh_available_devices();
                        this.set_status(err);
                    }
                }
            }
        });
        runtime.spawn(async move {
            let result = session
                .list_devices(DeviceModel::SoundcoreA3959)
                .await
                .map_err(|err| format!("{err:#}"));
            done(result);
        });
    }

    fn refresh_available_devices(&mut self) {
        let mut list = QVariantList::default();
        for descriptor in &self.available_descriptors {
            let mut item = QVariantMap::default();
            put_str(&mut item, "name", &descriptor.name);
            put_str(&mut item, "mac", &descriptor.mac_address.to_string());
            list.push(QVariant::from(item));
        }
        self.available_devices = list;
        self.available_devices_changed();
    }

    fn pair_and_connect(&mut self, mac: String, demo: bool) {
        let Some(descriptor) = self
            .available_descriptors
            .iter()
            .find(|descriptor| descriptor.mac_address.to_string() == mac)
            .cloned()
        else {
            self.set_status(format!("Device {mac} is no longer visible; scan again"));
            return;
        };

        let paired_device = PairedDevice {
            mac_address: descriptor.mac_address,
            model: DeviceModel::SoundcoreA3959,
            is_demo: demo,
        };

        self.set_busy(true);
        self.set_state("connecting");
        self.set_status("Pairing…");

        let session = self.session.clone();
        let runtime = self.runtime.clone();
        let qptr = QPointer::from(&*self);
        let done = queued_callback(move |event: ConnectEvent| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow_mut().handle_connect_event(event);
            }
        });
        runtime.spawn(async move {
            let event = async {
                session
                    .pair(paired_device)
                    .await
                    .map_err(|err| format!("{err:#}"))?;
                session
                    .connect(paired_device.mac_address)
                    .await
                    .map_err(|err| format!("{err:#}"))
            }
            .await;
            let event = match event {
                Ok(device) => ConnectEvent::Connected(device),
                Err(err) => ConnectEvent::Failed(err),
            };
            done(event);
        });
    }

    fn disconnect(&mut self) {
        self.current_device = None;
        self.device_name = String::new();
        self.device_name_changed();
        self.anc_mode = String::new();
        self.anc_mode_changed();
        self.set_state("disconnected");
        self.set_status("Disconnected");
        self.tray.update(|tray| tray.set_device(None));
        self.refresh_all();
    }

    fn set_category(&mut self, id: String) {
        if self.current_category == id {
            return;
        }
        self.current_category = id;
        self.current_category_changed();
        if let Some(device) = self.current_device.clone() {
            self.rebuild_settings(device.as_ref());
        }
    }

    fn set_toggle(&mut self, id: String, value: bool) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        self.send_setting(device, vec![(setting_id, Value::Bool(value))]);
    }

    fn set_select(&mut self, id: String, value: String) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        let Some(value) = select_value(device.as_ref(), setting_id, value) else {
            return;
        };
        self.send_setting(device, vec![(setting_id, value)]);
    }

    fn set_range(&mut self, id: String, value: i32) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        self.send_setting(device, vec![(setting_id, Value::I32(value))]);
    }

    fn set_equalizer_band(&mut self, id: String, index: i32, value: i32) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        let Some(Setting::Equalizer { value: current, .. }) = device.setting(&setting_id) else {
            return;
        };
        if index < 0 || index as usize >= current.len() {
            return;
        }
        let mut new_values = current.clone();
        new_values[index as usize] = value as i16;
        self.send_setting(device, vec![(setting_id, Value::I16Vec(new_values))]);
    }

    fn trigger_action(&mut self, id: String) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        self.send_setting(device, vec![(setting_id, Value::Bool(true))]);
    }

    fn set_anc_mode(&mut self, mode: String) {
        self.set_select("ambientSoundMode".to_string(), mode);
    }

    fn set_equalizer_preset(&mut self, preset: String) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let value = Value::OptionalString(Some(Cow::Owned(preset)));
        self.send_setting(device, vec![(SettingId::PresetEqualizerProfile, value)]);
    }

    fn quit(&mut self) {
        qmetaobject::qtcore::core_application::QCoreApplication::quit();
    }

    /// Handles a command dispatched from the system tray (invoked on the Qt thread).
    pub fn handle_tray_command(&mut self, command: TrayCommand) {
        match command {
            TrayCommand::SetAmbientSoundMode(mode) => self.set_anc_mode(mode),
            TrayCommand::SetEqualizerPreset(preset) => self.set_equalizer_preset(preset),
            TrayCommand::OpenSettings => self.open_requested(),
            TrayCommand::Quit => {
                qmetaobject::qtcore::core_application::QCoreApplication::quit();
            }
        }
    }
}

// --- value extraction helpers ---

fn information_value(device: &dyn OpenSCQ30Device, setting_id: SettingId) -> Option<String> {
    match device.setting(&setting_id)? {
        Setting::Information {
            translated_value, ..
        } => Some(translated_value),
        _ => None,
    }
}

fn current_select_value(device: &dyn OpenSCQ30Device, setting_id: SettingId) -> Option<String> {
    match device.setting(&setting_id)? {
        Setting::Select { value, .. } => Some(value.into_owned()),
        Setting::OptionalSelect { value, .. } => value.map(|value| value.into_owned()),
        Setting::ModifiableSelect { value, .. } => value.map(|value| value.into_owned()),
        Setting::PresetEqualizerProfileSelect { value, .. } => {
            value.map(|value| value.into_owned())
        }
        _ => None,
    }
}

fn select_value(
    device: &dyn OpenSCQ30Device,
    setting_id: SettingId,
    value: String,
) -> Option<Value> {
    match device.setting(&setting_id)? {
        Setting::Select { .. } => Some(Value::String(Cow::Owned(value))),
        Setting::OptionalSelect { .. }
        | Setting::ModifiableSelect { .. }
        | Setting::PresetEqualizerProfileSelect { .. } => {
            if value.is_empty() {
                Some(Value::OptionalString(None))
            } else {
                Some(Value::OptionalString(Some(Cow::Owned(value))))
            }
        }
        _ => None,
    }
}

// --- settings -> QML marshalling ---

fn setting_to_map(setting_id: SettingId, setting: &Setting) -> QVariantMap {
    let mut map = QVariantMap::default();
    put_str(&mut map, "id", &setting_id.to_string());
    put_str(&mut map, "label", &label_for_setting(setting_id));

    match setting {
        Setting::Toggle { value } => {
            put_str(&mut map, "kind", "toggle");
            put(&mut map, "value", QVariant::from(*value));
        }
        Setting::I32Range { setting, value } => {
            put_str(&mut map, "kind", "range");
            put(&mut map, "value", QVariant::from(*value));
            put(&mut map, "min", QVariant::from(*setting.range.start()));
            put(&mut map, "max", QVariant::from(*setting.range.end()));
            put(&mut map, "step", QVariant::from(setting.step));
        }
        Setting::Select { setting, value } => {
            put_str(&mut map, "kind", "select");
            put_str(&mut map, "value", value);
            put_select_options(&mut map, setting);
            put(&mut map, "nullable", QVariant::from(false));
        }
        Setting::OptionalSelect { setting, value }
        | Setting::PresetEqualizerProfileSelect {
            select: setting,
            value,
            ..
        }
        | Setting::ModifiableSelect { setting, value } => {
            put_str(&mut map, "kind", "select");
            put_str(&mut map, "value", value.as_deref().unwrap_or(""));
            put_select_options(&mut map, setting);
            put(&mut map, "nullable", QVariant::from(true));
        }
        Setting::MultiSelect { setting, values }
        | Setting::MultiSelectWithRemove { setting, values } => {
            put_str(&mut map, "kind", "multiselect");
            let mut list = QVariantList::default();
            for value in values {
                list.push(QVariant::from(QString::from(value.as_ref())));
            }
            put(&mut map, "values", QVariant::from(list));
            put_select_options(&mut map, setting);
        }
        Setting::Equalizer {
            setting,
            read_only,
            value,
        } => {
            put_str(&mut map, "kind", "equalizer");
            let mut values = QVariantList::default();
            for band in value {
                values.push(QVariant::from(*band as i32));
            }
            put(&mut map, "values", QVariant::from(values));
            put(&mut map, "min", QVariant::from(setting.min as i32));
            put(&mut map, "max", QVariant::from(setting.max as i32));
            let mut bands = QVariantList::default();
            for hz in setting.band_hz.iter() {
                bands.push(QVariant::from(*hz as i32));
            }
            put(&mut map, "bands", QVariant::from(bands));
            put(&mut map, "readOnly", QVariant::from(*read_only));
        }
        Setting::Information {
            value: _,
            translated_value,
        } => {
            put_str(&mut map, "kind", "information");
            put_str(&mut map, "value", translated_value);
        }
        Setting::ImportString { .. } => {
            put_str(&mut map, "kind", "import");
        }
        Setting::HueColorPicker { hue } => {
            put_str(&mut map, "kind", "hue");
            put(&mut map, "value", QVariant::from(*hue));
        }
        Setting::Action => {
            put_str(&mut map, "kind", "action");
        }
    }
    map
}

fn put_select_options(map: &mut QVariantMap, select: &Select) {
    let mut options = QVariantList::default();
    for option in &select.options {
        options.push(QVariant::from(QString::from(option.as_ref())));
    }
    let mut labels = QVariantList::default();
    for label in &select.localized_options {
        labels.push(QVariant::from(QString::from(label.as_str())));
    }
    put(map, "options", QVariant::from(options));
    put(map, "labels", QVariant::from(labels));
}

fn put(map: &mut QVariantMap, key: &str, value: QVariant) {
    map.insert(QString::from(key), value);
}

fn put_str(map: &mut QVariantMap, key: &str, value: &str) {
    map.insert(QString::from(key), QVariant::from(QString::from(value)));
}

// --- labels ---

fn label_for_category(category: CategoryId) -> String {
    match category {
        CategoryId::General => "General",
        CategoryId::SoundModes => "Sound Modes",
        CategoryId::Equalizer => "Equalizer",
        CategoryId::EqualizerImportExport => "Equalizer Import/Export",
        CategoryId::ButtonConfiguration => "Controls",
        CategoryId::DeviceInformation => "Device Information",
        CategoryId::Miscellaneous => "Miscellaneous",
        CategoryId::LimitHighVolume => "Volume Limit",
        CategoryId::DualConnections => "Dual Connections",
        CategoryId::Case => "Case",
        CategoryId::Lights => "Lights",
    }
    .to_string()
}

fn label_for_setting(setting_id: SettingId) -> String {
    match setting_id {
        SettingId::AmbientSoundMode => "Sound Mode",
        SettingId::NoiseCancelingMode => "Noise Cancelling Mode",
        SettingId::AdaptiveNoiseCanceling => "Adaptive Noise Cancelling",
        SettingId::ManualNoiseCanceling => "Manual Noise Cancelling",
        SettingId::WindNoiseSuppression => "Wind Noise Suppression",
        SettingId::WindNoiseDetected => "Wind Noise Detected",
        SettingId::AdaptiveNoiseCancelingSensitivityLevel => "ANC Sensitivity",
        SettingId::MultiSceneNoiseCanceling => "ANC Scene",
        SettingId::VolumeAdjustments => "Equalizer",
        SettingId::PresetEqualizerProfile => "Preset Equalizer",
        SettingId::CustomEqualizerProfile => "Custom Equalizer",
        SettingId::LeftSinglePress => "Left Single Press",
        SettingId::LeftDoublePress => "Left Double Press",
        SettingId::LeftTriplePress => "Left Triple Press",
        SettingId::LeftLongPress => "Left Long Press",
        SettingId::RightSinglePress => "Right Single Press",
        SettingId::RightDoublePress => "Right Double Press",
        SettingId::RightTriplePress => "Right Triple Press",
        SettingId::RightLongPress => "Right Long Press",
        SettingId::NormalModeInCycle => "Normal in Cycle",
        SettingId::TransparencyModeInCycle => "Transparency in Cycle",
        SettingId::NoiseCancelingModeInCycle => "Noise Cancelling in Cycle",
        SettingId::ResetButtonsToDefault => "Reset Button Settings",
        SettingId::DualConnections => "Dual Connections",
        SettingId::AutoPowerOff => "Auto Power Off",
        SettingId::TouchTone => "Touch Tone",
        SettingId::TwsStatus => "TWS Status",
        SettingId::HostDevice => "Connected Device",
        SettingId::LowBatteryPrompt => "Low Battery Prompt",
        SettingId::GamingMode => "Gaming Mode",
        SettingId::BatteryLevelLeft => "Left Battery",
        SettingId::BatteryLevelRight => "Right Battery",
        SettingId::IsChargingLeft => "Left Charging",
        SettingId::IsChargingRight => "Right Charging",
        SettingId::SerialNumber => "Serial Number",
        SettingId::FirmwareVersion => "Firmware Version",
        SettingId::FirmwareVersionLeft => "Left Firmware",
        SettingId::FirmwareVersionRight => "Right Firmware",
        _ => return setting_id.to_string().to_title_case(),
    }
    .to_string()
}
