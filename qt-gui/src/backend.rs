use std::{borrow::Cow, str::FromStr, sync::Arc};

use heck::ToTitleCase;
use openscq30_lib::{
    DeviceModel, OpenSCQ30Session,
    connection::{ConnectionDescriptor, ConnectionStatus},
    device::OpenSCQ30Device,
    settings::{CategoryId, MultiSelectWithRemoveCommand, Select, Setting, SettingId, Value},
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
#[allow(non_snake_case)]
pub struct Backend {
    base: qt_base_class!(trait QObject),

    // Runtime plumbing (not exposed to QML).
    runtime: tokio::runtime::Handle,
    session: Arc<OpenSCQ30Session>,
    current_device: Option<Arc<dyn OpenSCQ30Device + Send + Sync>>,
    available_descriptors: Vec<ConnectionDescriptor>,
    tray: TrayHandle,

    // Connection state.
    state: qt_property!(String; NOTIFY stateChanged),
    stateChanged: qt_signal!(),
    deviceName: qt_property!(String; NOTIFY deviceNameChanged),
    deviceNameChanged: qt_signal!(),
    statusMessage: qt_property!(String; NOTIFY statusMessageChanged),
    statusMessageChanged: qt_signal!(),

    // Battery.
    batteryLeft: qt_property!(String; NOTIFY batteryChanged),
    batteryRight: qt_property!(String; NOTIFY batteryChanged),
    chargingLeft: qt_property!(bool; NOTIFY batteryChanged),
    chargingRight: qt_property!(bool; NOTIFY batteryChanged),
    batteryChanged: qt_signal!(),

    // Headline ANC control.
    ancMode: qt_property!(String; NOTIFY ancModeChanged),
    ancModeChanged: qt_signal!(),

    // Pairing list.
    availableDevices: qt_property!(QVariantList; NOTIFY availableDevicesChanged),
    availableDevicesChanged: qt_signal!(),

    // Settings browsing.
    categories: qt_property!(QVariantList; NOTIFY categoriesChanged),
    categoriesChanged: qt_signal!(),
    currentCategory: qt_property!(String; NOTIFY currentCategoryChanged),
    currentCategoryChanged: qt_signal!(),
    settings: qt_property!(QVariantList; NOTIFY settingsChanged),
    settingsChanged: qt_signal!(),

    busy: qt_property!(bool; NOTIFY busyChanged),
    busyChanged: qt_signal!(),

    // Quick toggles.
    gamingMode: qt_property!(bool; NOTIFY gamingModeChanged),
    gamingModeChanged: qt_signal!(),
    dualConnections: qt_property!(bool; NOTIFY dualConnectionsChanged),
    dualConnectionsChanged: qt_signal!(),
    dualConnectionDevices: qt_property!(QVariantList; NOTIFY dualConnectionsChanged),
    touchTone: qt_property!(bool; NOTIFY touchToneChanged),
    touchToneChanged: qt_signal!(),
    lowBatteryPrompt: qt_property!(bool; NOTIFY lowBatteryPromptChanged),
    lowBatteryPromptChanged: qt_signal!(),
    windNoiseSuppression: qt_property!(bool; NOTIFY windNoiseSuppressionChanged),
    windNoiseSuppressionChanged: qt_signal!(),

    // Auto power off (select).
    autoPowerOff: qt_property!(String; NOTIFY autoPowerOffChanged),
    autoPowerOffChanged: qt_signal!(),
    autoPowerOffOptions: qt_property!(QVariantList; NOTIFY autoPowerOffChanged),
    autoPowerOffIndex: qt_property!(i32; NOTIFY autoPowerOffChanged),

    // Equalizer.
    eqBands: qt_property!(QVariantList; NOTIFY eqBandsChanged),
    eqBandsChanged: qt_signal!(),
    eqMin: qt_property!(i32; NOTIFY eqBandsChanged),
    eqMax: qt_property!(i32; NOTIFY eqBandsChanged),
    eqBandHz: qt_property!(QVariantList; NOTIFY eqBandsChanged),
    eqPreset: qt_property!(String; NOTIFY eqPresetChanged),
    eqPresetChanged: qt_signal!(),
    eqPresets: qt_property!(QVariantList; NOTIFY eqPresetChanged),
    eqPresetIndex: qt_property!(i32; NOTIFY eqPresetChanged),

    // Device information.
    serialNumber: qt_property!(String; NOTIFY infoChanged),
    firmwareVersion: qt_property!(String; NOTIFY infoChanged),
    firmwareVersionLeft: qt_property!(String; NOTIFY infoChanged),
    firmwareVersionRight: qt_property!(String; NOTIFY infoChanged),
    twsStatus: qt_property!(String; NOTIFY infoChanged),
    hostDevice: qt_property!(String; NOTIFY infoChanged),
    infoChanged: qt_signal!(),

    // Sound modes.
    noiseCancelingMode: qt_property!(String; NOTIFY soundModesChanged),
    noiseCancelingModeOptions: qt_property!(QVariantList; NOTIFY soundModesChanged),
    noiseCancelingModeIndex: qt_property!(i32; NOTIFY soundModesChanged),
    multiSceneNoiseCanceling: qt_property!(String; NOTIFY soundModesChanged),
    multiSceneNoiseCancelingOptions: qt_property!(QVariantList; NOTIFY soundModesChanged),
    multiSceneNoiseCancelingIndex: qt_property!(i32; NOTIFY soundModesChanged),
    manualNoiseCanceling: qt_property!(i32; NOTIFY soundModesChanged),
    manualNoiseCancelingMin: qt_property!(i32; NOTIFY soundModesChanged),
    manualNoiseCancelingMax: qt_property!(i32; NOTIFY soundModesChanged),
    adaptiveNoiseCanceling: qt_property!(String; NOTIFY soundModesChanged),
    ancSensitivity: qt_property!(i32; NOTIFY soundModesChanged),
    ancSensitivityMin: qt_property!(i32; NOTIFY soundModesChanged),
    ancSensitivityMax: qt_property!(i32; NOTIFY soundModesChanged),
    soundModesChanged: qt_signal!(),

    // Button configuration.
    buttonActions: qt_property!(QVariantList; NOTIFY buttonConfigChanged),
    buttonValues: qt_property!(QVariantList; NOTIFY buttonConfigChanged),
    buttonValueIndexes: qt_property!(QVariantList; NOTIFY buttonConfigChanged),
    normalModeInCycle: qt_property!(bool; NOTIFY buttonConfigChanged),
    transparencyModeInCycle: qt_property!(bool; NOTIFY buttonConfigChanged),
    noiseCancelingModeInCycle: qt_property!(bool; NOTIFY buttonConfigChanged),
    buttonConfigChanged: qt_signal!(),

    // QML-invokable methods.
    startup: qt_method!(fn(&mut self)),
    listDevices: qt_method!(fn(&mut self)),
    pairAndConnect: qt_method!(fn(&mut self, mac: String, demo: bool)),
    disconnect: qt_method!(fn(&mut self)),
    setCategory: qt_method!(fn(&mut self, id: String)),
    setToggle: qt_method!(fn(&mut self, id: String, value: bool)),
    setSelect: qt_method!(fn(&mut self, id: String, value: String)),
    setSelectByIndex: qt_method!(fn(&mut self, id: String, index: i32)),
    setRange: qt_method!(fn(&mut self, id: String, value: i32)),
    setEqualizerBand: qt_method!(fn(&mut self, id: String, index: i32, value: i32)),
    triggerAction: qt_method!(fn(&mut self, id: String)),
    setAncMode: qt_method!(fn(&mut self, mode: String)),
    setDualConnectionDevice: qt_method!(fn(&mut self, mac: String, checked: bool)),
    removeDualConnectionDevice: qt_method!(fn(&mut self, mac: String)),
    quit: qt_method!(fn(&mut self)),

    // Tray signals.
    openRequested: qt_signal!(),
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
            stateChanged: Default::default(),
            deviceName: String::new(),
            deviceNameChanged: Default::default(),
            statusMessage: "Not connected".to_string(),
            statusMessageChanged: Default::default(),
            batteryLeft: String::new(),
            batteryRight: String::new(),
            chargingLeft: false,
            chargingRight: false,
            batteryChanged: Default::default(),
            ancMode: String::new(),
            ancModeChanged: Default::default(),
            availableDevices: QVariantList::default(),
            availableDevicesChanged: Default::default(),
            categories: QVariantList::default(),
            categoriesChanged: Default::default(),
            currentCategory: String::new(),
            currentCategoryChanged: Default::default(),
            settings: QVariantList::default(),
            settingsChanged: Default::default(),
            busy: false,
            busyChanged: Default::default(),
            gamingMode: false,
            gamingModeChanged: Default::default(),
            dualConnections: false,
            dualConnectionsChanged: Default::default(),
            dualConnectionDevices: QVariantList::default(),
            touchTone: false,
            touchToneChanged: Default::default(),
            lowBatteryPrompt: false,
            lowBatteryPromptChanged: Default::default(),
            windNoiseSuppression: false,
            windNoiseSuppressionChanged: Default::default(),
            autoPowerOff: String::new(),
            autoPowerOffChanged: Default::default(),
            autoPowerOffOptions: QVariantList::default(),
            autoPowerOffIndex: -1,
            eqBands: QVariantList::default(),
            eqBandsChanged: Default::default(),
            eqMin: 0,
            eqMax: 0,
            eqBandHz: QVariantList::default(),
            eqPreset: String::new(),
            eqPresetChanged: Default::default(),
            eqPresets: QVariantList::default(),
            eqPresetIndex: -1,
            serialNumber: String::new(),
            firmwareVersion: String::new(),
            firmwareVersionLeft: String::new(),
            firmwareVersionRight: String::new(),
            twsStatus: String::new(),
            hostDevice: String::new(),
            infoChanged: Default::default(),
            noiseCancelingMode: String::new(),
            noiseCancelingModeOptions: QVariantList::default(),
            noiseCancelingModeIndex: -1,
            multiSceneNoiseCanceling: String::new(),
            multiSceneNoiseCancelingOptions: QVariantList::default(),
            multiSceneNoiseCancelingIndex: -1,
            manualNoiseCanceling: 0,
            manualNoiseCancelingMin: 0,
            manualNoiseCancelingMax: 0,
            adaptiveNoiseCanceling: String::new(),
            ancSensitivity: 0,
            ancSensitivityMin: 0,
            ancSensitivityMax: 0,
            soundModesChanged: Default::default(),
            buttonActions: QVariantList::default(),
            buttonValues: QVariantList::default(),
            buttonValueIndexes: QVariantList::default(),
            normalModeInCycle: false,
            transparencyModeInCycle: false,
            noiseCancelingModeInCycle: false,
            buttonConfigChanged: Default::default(),
            startup: Default::default(),
            listDevices: Default::default(),
            pairAndConnect: Default::default(),
            disconnect: Default::default(),
            setCategory: Default::default(),
            setToggle: Default::default(),
            setSelect: Default::default(),
            setSelectByIndex: Default::default(),
            setRange: Default::default(),
            setEqualizerBand: Default::default(),
            triggerAction: Default::default(),
            setAncMode: Default::default(),
            setDualConnectionDevice: Default::default(),
            removeDualConnectionDevice: Default::default(),
            quit: Default::default(),
            openRequested: Default::default(),
        }
    }

    // --- synchronous state helpers (always run on the Qt thread) ---

    fn set_state(&mut self, state: &str) {
        if self.state != state {
            tracing::info!(old = %self.state, new = %state, "connection state changed");
            self.state = state.to_string();
            self.stateChanged();
        }
    }

    fn set_busy(&mut self, busy: bool) {
        if self.busy != busy {
            self.busy = busy;
            self.busyChanged();
        }
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.statusMessage = message.into();
        self.statusMessageChanged();
    }

    fn set_connected(&mut self, device: Arc<dyn OpenSCQ30Device + Send + Sync>) {
        self.current_device = Some(device.clone());
        self.deviceName = DEVICE_NAME.to_string();
        self.deviceNameChanged();
        self.set_state("connected");
        self.set_status("Connected");
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
                // Populate the pairing list right away so the user can pick their device.
                self.listDevices();
            }
        }
    }

    fn handle_disconnect(&mut self) {
        if self.current_device.is_some() {
            self.current_device = None;
            self.deviceName = String::new();
            self.deviceNameChanged();
            self.ancMode = String::new();
            self.ancModeChanged();
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
            self.categoriesChanged();
            self.currentCategory = String::new();
            self.currentCategoryChanged();
            self.settings = QVariantList::default();
            self.settingsChanged();
            self.batteryLeft = String::new();
            self.batteryRight = String::new();
            self.chargingLeft = false;
            self.chargingRight = false;
            self.batteryChanged();
            self.ancMode = String::new();
            self.ancModeChanged();
            return;
        };

        self.batteryLeft =
            information_value(device.as_ref(), SettingId::BatteryLevelLeft).unwrap_or_default();
        self.batteryRight =
            information_value(device.as_ref(), SettingId::BatteryLevelRight).unwrap_or_default();
        self.chargingLeft = information_value(device.as_ref(), SettingId::IsChargingLeft)
            .is_some_and(|value| value.eq_ignore_ascii_case("yes"));
        self.chargingRight = information_value(device.as_ref(), SettingId::IsChargingRight)
            .is_some_and(|value| value.eq_ignore_ascii_case("yes"));
        self.batteryChanged();

        self.ancMode =
            current_select_value(device.as_ref(), SettingId::AmbientSoundMode).unwrap_or_default();
        self.ancModeChanged();

        // Quick toggles.
        self.gamingMode = toggle_value(device.as_ref(), SettingId::GamingMode);
        self.gamingModeChanged();
        self.dualConnections = toggle_value(device.as_ref(), SettingId::DualConnections);
        self.dualConnectionDevices = dual_connection_devices(device.as_ref());
        self.dualConnectionsChanged();
        self.touchTone = toggle_value(device.as_ref(), SettingId::TouchTone);
        self.touchToneChanged();
        self.lowBatteryPrompt = toggle_value(device.as_ref(), SettingId::LowBatteryPrompt);
        self.lowBatteryPromptChanged();
        self.windNoiseSuppression = toggle_value(device.as_ref(), SettingId::WindNoiseSuppression);
        self.windNoiseSuppressionChanged();

        // Auto power off.
        self.autoPowerOff =
            current_select_value(device.as_ref(), SettingId::AutoPowerOff).unwrap_or_default();
        self.autoPowerOffOptions = select_options(device.as_ref(), SettingId::AutoPowerOff);
        self.autoPowerOffIndex = select_index(device.as_ref(), SettingId::AutoPowerOff);
        self.autoPowerOffChanged();

        // Equalizer.
        match device.setting(&SettingId::VolumeAdjustments) {
            Some(Setting::Equalizer { setting, value, .. }) => {
                let mut bands = QVariantList::default();
                for band in &value {
                    bands.push(QVariant::from(*band as i32));
                }
                let mut band_hz = QVariantList::default();
                for hz in setting.band_hz.iter() {
                    band_hz.push(QVariant::from(*hz as i32));
                }
                self.eqBands = bands;
                self.eqBandHz = band_hz;
                self.eqMin = setting.min as i32;
                self.eqMax = setting.max as i32;
                self.eqBandsChanged();
            }
            _ => {
                self.eqBands = QVariantList::default();
                self.eqBandHz = QVariantList::default();
                self.eqMin = 0;
                self.eqMax = 0;
                self.eqBandsChanged();
            }
        }
        self.eqPreset = current_select_value(device.as_ref(), SettingId::PresetEqualizerProfile)
            .unwrap_or_default();
        self.eqPresets = select_options(device.as_ref(), SettingId::PresetEqualizerProfile);
        self.eqPresetIndex = select_index(device.as_ref(), SettingId::PresetEqualizerProfile);
        self.eqPresetChanged();

        // Sound modes.
        self.noiseCancelingMode =
            current_select_value(device.as_ref(), SettingId::NoiseCancelingMode)
                .unwrap_or_default();
        self.noiseCancelingModeOptions =
            select_options(device.as_ref(), SettingId::NoiseCancelingMode);
        self.noiseCancelingModeIndex = select_index(device.as_ref(), SettingId::NoiseCancelingMode);
        self.multiSceneNoiseCanceling =
            current_select_value(device.as_ref(), SettingId::MultiSceneNoiseCanceling)
                .unwrap_or_default();
        self.multiSceneNoiseCancelingOptions =
            select_options(device.as_ref(), SettingId::MultiSceneNoiseCanceling);
        self.multiSceneNoiseCancelingIndex =
            select_index(device.as_ref(), SettingId::MultiSceneNoiseCanceling);
        let (value, min, max) = range_info(device.as_ref(), SettingId::ManualNoiseCanceling);
        self.manualNoiseCanceling = value;
        self.manualNoiseCancelingMin = min;
        self.manualNoiseCancelingMax = max;
        self.adaptiveNoiseCanceling =
            information_value(device.as_ref(), SettingId::AdaptiveNoiseCanceling)
                .unwrap_or_default();
        let (value, min, max) = range_info(
            device.as_ref(),
            SettingId::AdaptiveNoiseCancelingSensitivityLevel,
        );
        self.ancSensitivity = value;
        self.ancSensitivityMin = min;
        self.ancSensitivityMax = max;
        self.soundModesChanged();

        // Button configuration.
        self.buttonActions = select_options(device.as_ref(), SettingId::LeftSinglePress);
        let button_ids = [
            SettingId::LeftSinglePress,
            SettingId::RightSinglePress,
            SettingId::LeftDoublePress,
            SettingId::RightDoublePress,
            SettingId::LeftTriplePress,
            SettingId::RightTriplePress,
            SettingId::LeftLongPress,
            SettingId::RightLongPress,
        ];
        let mut button_values = QVariantList::default();
        let mut button_indexes = QVariantList::default();
        for id in button_ids {
            button_values.push(QVariant::from(QString::from(
                current_select_value(device.as_ref(), id).unwrap_or_default(),
            )));
            button_indexes.push(QVariant::from(select_index(device.as_ref(), id)));
        }
        self.buttonValues = button_values;
        self.buttonValueIndexes = button_indexes;
        self.normalModeInCycle = toggle_value(device.as_ref(), SettingId::NormalModeInCycle);
        self.transparencyModeInCycle =
            toggle_value(device.as_ref(), SettingId::TransparencyModeInCycle);
        self.noiseCancelingModeInCycle =
            toggle_value(device.as_ref(), SettingId::NoiseCancelingModeInCycle);
        self.buttonConfigChanged();

        // Device information.
        self.serialNumber =
            information_value(device.as_ref(), SettingId::SerialNumber).unwrap_or_default();
        self.firmwareVersion =
            information_value(device.as_ref(), SettingId::FirmwareVersion).unwrap_or_default();
        self.firmwareVersionLeft =
            information_value(device.as_ref(), SettingId::FirmwareVersionLeft).unwrap_or_default();
        self.firmwareVersionRight =
            information_value(device.as_ref(), SettingId::FirmwareVersionRight).unwrap_or_default();
        self.twsStatus =
            information_value(device.as_ref(), SettingId::TwsStatus).unwrap_or_default();
        self.hostDevice =
            information_value(device.as_ref(), SettingId::HostDevice).unwrap_or_default();
        self.infoChanged();

        self.rebuild_categories(device.as_ref());
        self.rebuild_settings(device.as_ref());

        // Keep the tray tooltip/menu in sync (battery, ANC mode).
        self.tray.update(|_| {});
    }

    fn rebuild_categories(&mut self, device: &dyn OpenSCQ30Device) {
        let categories = device.categories();
        if categories.is_empty() {
            self.categories = QVariantList::default();
            self.categoriesChanged();
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
        self.categoriesChanged();

        let valid = categories
            .iter()
            .any(|category| category.to_string() == self.currentCategory);
        if !valid {
            self.currentCategory = categories[0].to_string();
            self.currentCategoryChanged();
        }
    }

    fn rebuild_settings(&mut self, device: &dyn OpenSCQ30Device) {
        let Some(category) = device
            .categories()
            .into_iter()
            .find(|category| category.to_string() == self.currentCategory)
        else {
            self.settings = QVariantList::default();
            self.settingsChanged();
            return;
        };

        let mut list = QVariantList::default();
        for setting_id in device.settings_in_category(&category) {
            if let Some(setting) = device.setting(&setting_id) {
                list.push(QVariant::from(setting_to_map(setting_id, &setting)));
            }
        }
        self.settings = list;
        self.settingsChanged();
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
        // `watch_for_changes` spawns its own tokio task, so it must run inside the runtime.
        let watch_device = device.clone();
        runtime.spawn(async move {
            let mut watch = watch_device.watch_for_changes();
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
        let status_device = device.clone();
        runtime.spawn(async move {
            let mut status = status_device.connection_status();
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
        for (id, value) in &values {
            tracing::info!(setting = %id, value = %value, "setting change requested");
        }
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
            match &result {
                Ok(()) => tracing::info!("setting change applied"),
                Err(err) => tracing::warn!(error = %err, "setting change failed"),
            }
            done(result);
        });
    }
}

#[allow(non_snake_case)]
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

    fn listDevices(&mut self) {
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
        self.availableDevices = list;
        self.availableDevicesChanged();
    }

    fn pairAndConnect(&mut self, mac: String, demo: bool) {
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
        self.deviceName = String::new();
        self.deviceNameChanged();
        self.ancMode = String::new();
        self.ancModeChanged();
        self.set_state("disconnected");
        self.set_status("Disconnected");
        self.tray.update(|tray| tray.set_device(None));
        self.refresh_all();
    }

    fn setCategory(&mut self, id: String) {
        if self.currentCategory == id {
            return;
        }
        self.currentCategory = id;
        self.currentCategoryChanged();
        if let Some(device) = self.current_device.clone() {
            self.rebuild_settings(device.as_ref());
        }
    }

    fn setToggle(&mut self, id: String, value: bool) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        self.send_setting(device, vec![(setting_id, Value::Bool(value))]);
    }

    fn setSelect(&mut self, id: String, value: String) {
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

    /// Like `setSelect`, but takes the display index and maps it back to the raw
    /// option value, since QML shows localized labels while the device speaks raw
    /// option names.
    fn setSelectByIndex(&mut self, id: String, index: i32) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        let (Some(Setting::Select { setting, .. })
        | Some(Setting::OptionalSelect { setting, .. })
        | Some(Setting::ModifiableSelect { setting, .. })
        | Some(Setting::PresetEqualizerProfileSelect {
            select: setting, ..
        })) = device.setting(&setting_id)
        else {
            return;
        };
        let Some(raw) = setting.options.get(index as usize) else {
            return;
        };
        let Some(value) = select_value(device.as_ref(), setting_id, raw.to_string()) else {
            return;
        };
        self.send_setting(device, vec![(setting_id, value)]);
    }

    fn setRange(&mut self, id: String, value: i32) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        self.send_setting(device, vec![(setting_id, Value::I32(value))]);
    }

    fn setEqualizerBand(&mut self, id: String, index: i32, value: i32) {
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

    fn triggerAction(&mut self, id: String) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(setting_id) = SettingId::from_str(&id).ok() else {
            return;
        };
        self.send_setting(device, vec![(setting_id, Value::Bool(true))]);
    }

    fn setAncMode(&mut self, mode: String) {
        self.setSelect("ambientSoundMode".to_string(), mode);
    }

    /// Toggles one device in the dual-connections picker on/off.
    fn setDualConnectionDevice(&mut self, mac: String, checked: bool) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        let Some(Setting::MultiSelectWithRemove { values, .. }) =
            device.setting(&SettingId::DualConnectionsDevices)
        else {
            return;
        };
        let mut selected: Vec<Cow<'static, str>> = values
            .iter()
            .filter(|value| value.as_ref() != mac)
            .map(|value| Cow::Owned(value.to_string()))
            .collect();
        if checked {
            selected.push(Cow::Owned(mac));
        }
        self.send_setting(
            device,
            vec![(
                SettingId::DualConnectionsDevices,
                Value::StringVec(selected),
            )],
        );
    }

    /// Removes a device from the dual-connections list entirely.
    fn removeDualConnectionDevice(&mut self, mac: String) {
        let Some(device) = self.current_device.clone() else {
            return;
        };
        self.send_setting(
            device,
            vec![(
                SettingId::DualConnectionsDevices,
                Value::MultiSelectWithRemoveCommand(MultiSelectWithRemoveCommand::Remove(
                    Cow::Owned(mac),
                )),
            )],
        );
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
            TrayCommand::SetAmbientSoundMode(mode) => self.setAncMode(mode),
            TrayCommand::SetEqualizerPreset(preset) => self.set_equalizer_preset(preset),
            TrayCommand::OpenSettings => self.openRequested(),
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

fn toggle_value(device: &dyn OpenSCQ30Device, setting_id: SettingId) -> bool {
    matches!(
        device.setting(&setting_id),
        Some(Setting::Toggle { value: true })
    )
}

/// Returns `(value, min, max)` for an `I32Range` setting, defaulting to zeroes
/// when the setting is absent or not a range.
fn range_info(device: &dyn OpenSCQ30Device, setting_id: SettingId) -> (i32, i32, i32) {
    match device.setting(&setting_id) {
        Some(Setting::I32Range { setting, value }) => {
            (value, *setting.range.start(), *setting.range.end())
        }
        _ => (0, 0, 0),
    }
}

/// Builds a QML list of `{ name, mac, checked }` maps for the dual-connections
/// device picker from the `MultiSelectWithRemove` setting.
fn dual_connection_devices(device: &dyn OpenSCQ30Device) -> QVariantList {
    let Some(Setting::MultiSelectWithRemove { setting, values }) =
        device.setting(&SettingId::DualConnectionsDevices)
    else {
        return QVariantList::default();
    };

    let mut list = QVariantList::default();
    for (index, option) in setting.options.iter().enumerate() {
        let mut item = QVariantMap::default();
        put_str(&mut item, "mac", option);
        let name = setting
            .localized_options
            .get(index)
            .map(|name| name.as_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(option.as_ref());
        put_str(&mut item, "name", name);
        put(
            &mut item,
            "checked",
            QVariant::from(values.iter().any(|value| value.as_ref() == option.as_ref())),
        );
        list.push(QVariant::from(item));
    }
    list
}

/// Returns the raw option names for a select-style setting as a QML list.
/// Returns the localized option labels for a select-style setting, matching the
/// original GUI which renders `Select::localized_options`.
fn select_options(device: &dyn OpenSCQ30Device, setting_id: SettingId) -> QVariantList {
    let select = match device.setting(&setting_id) {
        Some(Setting::Select { setting, .. })
        | Some(Setting::OptionalSelect { setting, .. })
        | Some(Setting::ModifiableSelect { setting, .. })
        | Some(Setting::PresetEqualizerProfileSelect {
            select: setting, ..
        }) => Some(setting),
        _ => None,
    };
    let Some(select) = select else {
        return QVariantList::default();
    };
    select
        .localized_options
        .iter()
        .map(|label| QVariant::from(QString::from(label.as_str())))
        .collect()
}

/// Returns the index of the currently selected option for a select-style
/// setting, or `-1` when the value is unset. Computed in Rust so QML never has
/// to compare QString-backed values with `===`.
fn select_index(device: &dyn OpenSCQ30Device, setting_id: SettingId) -> i32 {
    let position = |select: &Select, value: Option<&str>| {
        let Some(value) = value else {
            return -1;
        };
        select
            .options
            .iter()
            .position(|option| option.as_ref() == value)
            .map_or(-1, |index| index as i32)
    };
    match device.setting(&setting_id) {
        Some(Setting::Select { setting, value }) => position(&setting, Some(&value)),
        Some(Setting::OptionalSelect { setting, value })
        | Some(Setting::ModifiableSelect { setting, value })
        | Some(Setting::PresetEqualizerProfileSelect {
            select: setting,
            value,
            ..
        }) => position(&setting, value.as_deref()),
        _ => -1,
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
