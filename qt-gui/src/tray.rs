use std::sync::Arc;

use ksni::{
    Category, Handle, Icon, MenuItem, ToolTip, Tray,
    menu::{Disposition, RadioGroup, RadioItem, StandardItem, SubMenu},
};
use openscq30_lib::{
    device::OpenSCQ30Device,
    settings::{Setting, SettingId},
};
use tokio::sync::mpsc::UnboundedSender;

/// Commands sent from the tray's D-Bus thread to the Qt event loop.
#[derive(Debug, Clone)]
pub enum TrayCommand {
    SetAmbientSoundMode(String),
    SetEqualizerPreset(String),
    OpenSettings,
    Quit,
}

/// The ANC modes supported by the R50i NC, in the order they appear in the tray radio group.
const ANC_MODES: [&str; 3] = ["Normal", "Transparency", "NoiseCanceling"];
const ANC_LABELS: [&str; 3] = ["Normal", "Transparency", "Noise Cancelling"];

/// Pre-rendered ARGB32 copies of the app icon (see `resources/tray-*.argb`).
const ICON_24: &[u8] = include_bytes!("../resources/tray-24.argb");
const ICON_48: &[u8] = include_bytes!("../resources/tray-48.argb");
const ICON_96: &[u8] = include_bytes!("../resources/tray-96.argb");

pub struct TrayModel {
    device: Option<Arc<dyn OpenSCQ30Device + Send + Sync>>,
    command_tx: UnboundedSender<TrayCommand>,
}

impl TrayModel {
    pub fn new(command_tx: UnboundedSender<TrayCommand>) -> Self {
        Self {
            device: None,
            command_tx,
        }
    }

    pub fn set_device(&mut self, device: Option<Arc<dyn OpenSCQ30Device + Send + Sync>>) {
        self.device = device;
    }

    fn ambient_sound_mode(&self) -> Option<String> {
        match self
            .device
            .as_ref()?
            .setting(&SettingId::AmbientSoundMode)?
        {
            Setting::Select { value, .. } => Some(value.into_owned()),
            _ => None,
        }
    }
}

fn information_value(
    device: &(dyn OpenSCQ30Device + Send + Sync),
    setting_id: &SettingId,
) -> Option<String> {
    match device.setting(setting_id)? {
        Setting::Information {
            translated_value, ..
        } => Some(translated_value),
        _ => None,
    }
}

/// Turns a "5/10"-style battery value into "50%" (or returns the raw text when it isn't a fraction).
fn battery_percentage(
    device: &(dyn OpenSCQ30Device + Send + Sync),
    setting_id: &SettingId,
) -> Option<String> {
    let value = information_value(device, setting_id)?;
    let Some((num, den)) = value.split_once('/') else {
        return Some(value);
    };
    let (Ok(num), Ok(den)) = (num.trim().parse::<f64>(), den.trim().parse::<f64>()) else {
        return Some(value);
    };
    if den <= 0.0 {
        return Some(value);
    }
    Some(format!("{:.0}%", num / den * 100.0))
}

/// Returns `(raw_options, display_labels, selected_index)` for the equalizer preset selector.
fn equalizer_presets(
    device: &(dyn OpenSCQ30Device + Send + Sync),
) -> Option<(Vec<String>, Vec<String>, usize)> {
    match device.setting(&SettingId::PresetEqualizerProfile)? {
        Setting::PresetEqualizerProfileSelect { select, value, .. } => {
            let raw: Vec<String> = select
                .options
                .iter()
                .map(|option| option.to_string())
                .collect();
            let selected = value
                .as_deref()
                .and_then(|selected| select.options.iter().position(|option| option == selected))
                .unwrap_or(0);
            Some((raw, select.localized_options, selected))
        }
        _ => None,
    }
}

impl Tray for TrayModel {
    fn id(&self) -> String {
        "com.oppzippy.OpenSCQ30".to_string()
    }

    fn title(&self) -> String {
        "OpenSCQ30".to_string()
    }

    fn icon_name(&self) -> String {
        "com.oppzippy.OpenSCQ30".to_string()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![
            Icon {
                width: 24,
                height: 24,
                data: ICON_24.to_vec(),
            },
            Icon {
                width: 48,
                height: 48,
                data: ICON_48.to_vec(),
            },
            Icon {
                width: 96,
                height: 96,
                data: ICON_96.to_vec(),
            },
        ]
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "OpenSCQ30".to_string(),
            description: self.device.as_ref().map_or_else(
                || "Not connected".to_string(),
                |device| {
                    let left = battery_percentage(device.as_ref(), &SettingId::BatteryLevelLeft);
                    let right = battery_percentage(device.as_ref(), &SettingId::BatteryLevelRight);
                    match (left, right) {
                        (Some(left), Some(right)) => format!("Left {left} · Right {right}"),
                        (Some(value), None) | (None, Some(value)) => {
                            format!("Battery {value}")
                        }
                        (None, None) => "Not connected".to_string(),
                    }
                },
            ),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items = Vec::new();

        let Some(device) = self.device.as_ref() else {
            items.push(MenuItem::Standard(StandardItem {
                label: "Not connected".to_string(),
                enabled: false,
                disposition: Disposition::Informative,
                ..Default::default()
            }));
            items.push(MenuItem::Separator);
            items.push(open_settings_item());
            items.push(quit_item());
            return items;
        };

        // Battery levels, shown separately per earbud.
        let left = battery_percentage(device.as_ref(), &SettingId::BatteryLevelLeft);
        let right = battery_percentage(device.as_ref(), &SettingId::BatteryLevelRight);
        if let Some(left) = left {
            items.push(MenuItem::Standard(StandardItem {
                label: format!("Left battery: {left}"),
                enabled: false,
                disposition: Disposition::Informative,
                ..Default::default()
            }));
        }
        if let Some(right) = right {
            items.push(MenuItem::Standard(StandardItem {
                label: format!("Right battery: {right}"),
                enabled: false,
                disposition: Disposition::Informative,
                ..Default::default()
            }));
        }
        if !items.is_empty() {
            items.push(MenuItem::Separator);
        }

        // ANC / sound mode.
        let selected = self
            .ambient_sound_mode()
            .and_then(|mode| ANC_MODES.iter().position(|candidate| *candidate == mode))
            .unwrap_or(0);
        items.push(MenuItem::RadioGroup(RadioGroup {
            selected,
            select: Box::new(|tray: &mut Self, index| {
                if let Some(mode) = ANC_MODES.get(index) {
                    let _ = tray
                        .command_tx
                        .send(TrayCommand::SetAmbientSoundMode((*mode).to_string()));
                }
            }),
            options: ANC_LABELS
                .iter()
                .map(|label| RadioItem {
                    label: (*label).to_string(),
                    ..Default::default()
                })
                .collect(),
        }));

        // Equalizer preset selector (only when the device exposes one).
        // Wrapped in a submenu so it collapses to one row instead of a long radio group.
        if let Some((raw, labels, selected)) = equalizer_presets(device.as_ref()) {
            items.push(MenuItem::Separator);
            items.push(MenuItem::SubMenu(SubMenu {
                label: "Equalizer preset".to_string(),
                icon_name: "view-media-equalizer".to_string(),
                submenu: vec![MenuItem::RadioGroup(RadioGroup {
                    selected,
                    select: Box::new(move |tray: &mut Self, index| {
                        if let Some(preset) = raw.get(index) {
                            let _ = tray
                                .command_tx
                                .send(TrayCommand::SetEqualizerPreset(preset.clone()));
                        }
                    }),
                    options: labels
                        .into_iter()
                        .map(|label| RadioItem {
                            label,
                            ..Default::default()
                        })
                        .collect(),
                })],
                ..Default::default()
            }));
        }

        items.push(MenuItem::Separator);
        items.push(open_settings_item());
        items.push(quit_item());
        items
    }
}

fn open_settings_item() -> MenuItem<TrayModel> {
    MenuItem::Standard(StandardItem {
        label: "Open Settings".to_string(),
        icon_name: "preferences-system".to_string(),
        activate: Box::new(|tray: &mut TrayModel| {
            let _ = tray.command_tx.send(TrayCommand::OpenSettings);
        }),
        ..Default::default()
    })
}

fn quit_item() -> MenuItem<TrayModel> {
    MenuItem::Standard(StandardItem {
        label: "Quit".to_string(),
        icon_name: "application-exit".to_string(),
        activate: Box::new(|tray: &mut TrayModel| {
            let _ = tray.command_tx.send(TrayCommand::Quit);
        }),
        ..Default::default()
    })
}

pub type TrayHandle = Handle<TrayModel>;

/// Starts the tray service on a background thread and returns a handle to it.
pub fn spawn(command_tx: UnboundedSender<TrayCommand>) -> TrayHandle {
    let service = ksni::TrayService::new(TrayModel::new(command_tx));
    let handle = service.handle();
    std::thread::spawn(move || {
        if let Err(err) = service.run() {
            tracing::error!("system tray service stopped: {err}");
        }
    });
    handle
}
