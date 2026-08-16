use std::sync::Arc;

use ksni::{
    Category, Handle, MenuItem, ToolTip, Tray,
    menu::{Disposition, RadioGroup, RadioItem, StandardItem},
};
use openscq30_lib::{
    device::OpenSCQ30Device,
    settings::{Setting, SettingId},
};
use tokio::sync::mpsc::UnboundedSender;

/// Commands sent from the tray's D-Bus thread to the GUI's event loop.
#[derive(Debug, Clone)]
pub enum TrayCommand {
    SetAmbientSoundMode(String),
    OpenSettings,
    Quit,
}

/// The single supported ANC modes, in the order they appear in the tray radio group.
const ANC_MODES: [&str; 3] = ["Normal", "Transparency", "NoiseCanceling"];
const ANC_LABELS: [&str; 3] = ["Normal", "Transparency", "Noise Cancelling"];

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

    fn battery(&self) -> Option<String> {
        let device = self.device.as_ref()?;
        let left = information_value(device.as_ref(), &SettingId::BatteryLevelLeft);
        let right = information_value(device.as_ref(), &SettingId::BatteryLevelRight);
        match (left, right) {
            (Some(left), Some(right)) => Some(format!("{left} / {right}")),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
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

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "OpenSCQ30".to_string(),
            description: self
                .battery()
                .map(|battery| format!("Battery: {battery}"))
                .unwrap_or_else(|| "Not connected".to_string()),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items = Vec::new();

        if let Some(battery) = self.battery() {
            items.push(MenuItem::Standard(StandardItem {
                label: format!("Battery: {battery}"),
                enabled: false,
                disposition: Disposition::Informative,
                ..Default::default()
            }));
            items.push(MenuItem::Separator);
        }

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

        items.push(MenuItem::Separator);

        items.push(MenuItem::Standard(StandardItem {
            label: "Open Settings".to_string(),
            icon_name: "preferences-system".to_string(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.command_tx.send(TrayCommand::OpenSettings);
            }),
            ..Default::default()
        }));

        items.push(MenuItem::Standard(StandardItem {
            label: "Quit".to_string(),
            icon_name: "application-exit".to_string(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.command_tx.send(TrayCommand::Quit);
            }),
            ..Default::default()
        }));

        items
    }
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
