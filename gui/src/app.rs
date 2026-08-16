use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    iter,
    ops::Deref,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};

use cosmic::{
    Application, ApplicationExt, Apply, Task,
    app::{Core, context_drawer::ContextDrawer},
    iced::{Length, alignment, event, keyboard, window},
    widget::{self, menu::KeyBind, nav_bar},
};
use i18n_embed::unic_langid::LanguageIdentifier;
use openscq30_i18n::Translate;
#[cfg(target_os = "linux")]
use openscq30_lib::settings::{SettingId, Value};
use openscq30_lib::{
    DeviceModel, OpenSCQ30Session, device::OpenSCQ30Device, storage::PairedDevice,
};
use tokio::{select, sync::Semaphore};

#[cfg(target_os = "linux")]
use crate::tray::{TrayCommand, TrayHandle};
use crate::{
    add_device::{self, AddDeviceModel},
    config::Config,
    device_settings, fl,
    utils::coalesce_result,
};

pub struct AppModel {
    core: Core,
    screen: Screen,
    session: Arc<OpenSCQ30Session>,
    warnings: VecDeque<String>,
    config: Config,
    config_dir: PathBuf,
    about: widget::about::About,
    context_drawer_screen: Option<ContextDrawerScreen>,
    available_language_names: Vec<Cow<'static, str>>,
    available_languages: Vec<Option<LanguageIdentifier>>,
    key_binds: HashMap<KeyBind, KeyBindAction>,
    #[cfg(target_os = "linux")]
    tray: TrayHandle,
    #[cfg(target_os = "linux")]
    current_device: Option<Arc<dyn OpenSCQ30Device + Send + Sync>>,
}

#[derive(Clone, Copy)]
enum KeyBindAction {
    Settings,
}

pub struct AppFlags {
    pub config: Config,
    pub config_dir: PathBuf,
    #[cfg(target_os = "linux")]
    pub tray: TrayHandle,
    #[cfg(target_os = "linux")]
    pub tray_command_rx: tokio::sync::mpsc::UnboundedReceiver<TrayCommand>,
}

enum ContextDrawerScreen {
    About,
    Settings,
}

#[derive(Debug, Clone)]
pub enum Message {
    AddDeviceScreen(add_device::Message),
    DeviceSettingsScreen(device_settings::Message),
    ActivateConnectToDeviceScreen(DebugOpenSCQ30Device),
    ConnectToDevice(PairedDevice),
    ConnectToDeviceFailed(String),
    CancelConnectToDevice,
    ShowAddDevice,
    Warning(String),
    CloseWarning,
    ToggleAbout,
    OpenUrl(String),
    CloseContextDrawer,
    ToggleSettings,
    None,
    SetPreferredLanguage(usize),
    KeyPressed {
        modifiers: keyboard::Modifiers,
        key: keyboard::Key,
        physical_key: keyboard::key::Physical,
    },
    #[cfg(target_os = "linux")]
    TrayCommand(TrayCommand),
    #[cfg(target_os = "linux")]
    CloseToTray,
}

impl From<add_device::Message> for Message {
    fn from(message: add_device::Message) -> Self {
        Self::AddDeviceScreen(message)
    }
}
impl From<device_settings::Message> for Message {
    fn from(message: device_settings::Message) -> Self {
        Self::DeviceSettingsScreen(message)
    }
}

#[derive(Clone)]
pub struct DebugOpenSCQ30Device(pub Arc<dyn OpenSCQ30Device + Send + Sync>);
impl std::fmt::Debug for DebugOpenSCQ30Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenSCQ30Device").finish()
    }
}
impl Deref for DebugOpenSCQ30Device {
    type Target = Arc<dyn OpenSCQ30Device + Send + Sync>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[allow(clippy::large_enum_variant)]
enum Screen {
    AddDevice(add_device::AddDeviceModel),
    Connecting {
        canceled: Arc<Semaphore>,
        name: String,
    },
    DeviceSettings(device_settings::DeviceSettingsModel),
}

// This is a macro so that the file/line number of the tracing message matches the caller
#[macro_export]
macro_rules! handle_soft_error {
    () => {
        |err| {
            let err = ::anyhow::Error::from(err);
            ::tracing::warn!("soft_error: {err:?}");
            Message::Warning($crate::fl!("error-with-message", err = format!("{err:#}")))
        }
    };
}

/// Connects to a paired device, resolving to the device settings screen on success.
fn connect_to_device(
    session: Arc<OpenSCQ30Session>,
    paired_device: PairedDevice,
    canceled: Arc<Semaphore>,
) -> cosmic::app::Task<Message> {
    Task::future(async move {
        let connect_result = select! {
            connect_result = session.connect(paired_device.mac_address) => connect_result,
            _ = canceled.acquire() => return Ok(Message::None.into()),
        };

        match connect_result {
            Ok(device) => {
                Ok(Message::ActivateConnectToDeviceScreen(DebugOpenSCQ30Device(device)).into())
            }
            Err(err) => {
                let err = anyhow::Error::from(err);
                tracing::warn!("soft_error: {err:?}");
                Ok(Message::ConnectToDeviceFailed(fl!(
                    "error-with-message",
                    err = format!("{err:#}")
                ))
                .into())
            }
        }
    })
    .map(coalesce_result)
}

impl Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = AppFlags;
    type Message = Message;

    const APP_ID: &'static str = "com.oppzippy.OpenSCQ30";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, cosmic::app::Task<Self::Message>) {
        let AppFlags {
            config,
            config_dir,
            #[cfg(target_os = "linux")]
            tray,
            #[cfg(target_os = "linux")]
            tray_command_rx,
        } = flags;

        let about = widget::about::About::default()
            .name(fl!("openscq30"))
            .icon(crate::icons::openscq30())
            .version(env!("CARGO_PKG_VERSION"))
            .author(env!("CARGO_PKG_AUTHORS"))
            .license(env!("CARGO_PKG_LICENSE"))
            .links([(env!("CARGO_PKG_REPOSITORY"), env!("CARGO_PKG_REPOSITORY"))]);

        let session = Arc::new(
            futures::executor::block_on(OpenSCQ30Session::new(config_dir.join("database.sqlite")))
                .expect("database is required to run"),
        );
        let (add_device_model, add_device_task) = AddDeviceModel::new(session.clone());

        // Only the Soundcore R50i NC (A3959) is supported. Auto-select it and connect on launch
        // if it's already paired; otherwise show the pairing screen.
        let paired_devices =
            futures::executor::block_on(session.paired_devices()).unwrap_or_else(|err| {
                tracing::warn!("failed to load paired devices: {err:?}");
                Vec::new()
            });
        let paired_device = paired_devices
            .iter()
            .find(|device| device.model == DeviceModel::SoundcoreA3959)
            .or_else(|| paired_devices.first())
            .copied();

        let (screen, startup_task) = match paired_device {
            Some(device) => {
                let canceled = Arc::new(Semaphore::new(0));
                let connect_task = connect_to_device(session.clone(), device, canceled.clone());
                (
                    Screen::Connecting {
                        canceled,
                        name: device.model.translate(),
                    },
                    connect_task,
                )
            }
            None => (
                Screen::AddDevice(add_device_model),
                add_device_task
                    .map(Message::AddDeviceScreen)
                    .map(Into::into),
            ),
        };

        let (available_languages, available_language_names) =
            iter::once((None, Cow::Owned(fl!("default"))))
                .chain(
                    crate::i18n::languages()
                        .map(|(identifier, name)| (Some(identifier), Cow::Owned(name))),
                )
                .collect::<(Vec<_>, Vec<_>)>();
        let mut app = Self {
            core,
            screen,
            session,
            warnings: VecDeque::with_capacity(5),
            config,
            config_dir,
            about,
            context_drawer_screen: None,
            available_language_names,
            available_languages,
            key_binds: key_binds(),
            #[cfg(target_os = "linux")]
            tray,
            #[cfg(target_os = "linux")]
            current_device: None,
        };
        let command = app.update_title();

        #[cfg(target_os = "linux")]
        let tray_command_stream = {
            let mut rx = tray_command_rx;
            cosmic::iced::stream::channel(1, async move |mut output| {
                while let Some(command) = rx.recv().await {
                    if output.try_send(Message::TrayCommand(command)).is_err() {
                        break;
                    }
                }
            })
        };

        #[cfg(target_os = "linux")]
        let task = cosmic::Task::batch([
            command,
            startup_task,
            cosmic::Task::stream(tray_command_stream).map(Into::into),
        ]);
        #[cfg(not(target_os = "linux"))]
        let task = cosmic::Task::batch([command, startup_task]);
        (app, task)
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        event::listen_with(|event, status, _window_id| match event {
            #[cfg(target_os = "linux")]
            event::Event::Window(window::Event::CloseRequested) => Some(Message::CloseToTray),
            event::Event::Keyboard(cosmic::iced::keyboard::Event::KeyPressed {
                modifiers,
                key,
                physical_key,
                ..
            }) if matches!(status, cosmic::iced::event::Status::Ignored) => {
                Some(Message::KeyPressed {
                    modifiers,
                    key,
                    physical_key,
                })
            }
            _ => None,
        })
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        match &self.screen {
            Screen::DeviceSettings(model) => model.nav_model(),
            _ => None,
        }
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> cosmic::app::Task<Self::Message> {
        match &mut self.screen {
            Screen::DeviceSettings(model) => model
                .on_nav_select(id)
                .map(Message::DeviceSettingsScreen)
                .map(Into::into),
            _ => unreachable!("no nav bar is shown, so selecting an item is impossible"),
        }
    }

    fn header_end(&self) -> Vec<cosmic::Element<'_, Self::Message>> {
        vec![
            widget::button::icon(widget::icon::from_name("preferences-system-symbolic"))
                .on_press(Message::ToggleSettings)
                .into(),
            widget::button::icon(widget::icon::from_name("help-about-symbolic"))
                .on_press(Message::ToggleAbout)
                .into(),
        ]
    }

    fn view(&self) -> cosmic::Element<'_, Self::Message> {
        widget::Column::with_capacity(2)
            .push_maybe(
                self.warnings.front().map(|message| {
                    crate::warning::warning(message).on_close(Message::CloseWarning)
                }),
            )
            .push(match &self.screen {
                Screen::AddDevice(add_device_model) => {
                    add_device_model.view().map(Message::AddDeviceScreen)
                }
                Screen::Connecting { canceled: _, name } => self.view_cancel(name),
                Screen::DeviceSettings(device_settings_model) => device_settings_model
                    .view()
                    .map(Message::DeviceSettingsScreen),
            })
            .into()
    }

    fn dialog(&self) -> Option<cosmic::Element<'_, Self::Message>> {
        match &self.screen {
            Screen::AddDevice(_) => None,
            Screen::Connecting { .. } => None,
            Screen::DeviceSettings(device_settings_model) => device_settings_model
                .dialog()
                .map(|e| e.map(Message::DeviceSettingsScreen)),
        }
    }

    fn context_drawer(&self) -> Option<ContextDrawer<'_, Self::Message>> {
        if let Some(context_drawer_screen) = &self.context_drawer_screen {
            match context_drawer_screen {
                ContextDrawerScreen::About => Some(cosmic::app::context_drawer::about(
                    &self.about,
                    |url| Message::OpenUrl(url.to_owned()),
                    Message::CloseContextDrawer,
                )),
                ContextDrawerScreen::Settings => Some(
                    cosmic::app::context_drawer::context_drawer(
                        widget::column![
                            widget::settings::item::builder(fl!("preferred-language"))
                                .flex_control(widget::dropdown(
                                    &self.available_language_names,
                                    Some(
                                        self.config
                                            .get()
                                            .preferred_language
                                            .as_ref()
                                            .and_then(|preferred_language| {
                                                LanguageIdentifier::from_str(preferred_language)
                                                    .ok()
                                            })
                                            .and_then(|preferred_language| {
                                                self.available_languages
                                                    .iter()
                                                    .skip(1)
                                                    .position(|l| {
                                                        l.as_ref() == Some(&preferred_language)
                                                    })
                                                    .map(|index| index + 1)
                                            })
                                            .unwrap_or_default(),
                                    ),
                                    Message::SetPreferredLanguage,
                                )),
                        ],
                        Message::CloseContextDrawer,
                    )
                    .title(fl!("settings")),
                ),
            }
        } else {
            match &self.screen {
                Screen::AddDevice(_) => None,
                Screen::Connecting { .. } => None,
                Screen::DeviceSettings(device_settings_model) => device_settings_model
                    .context_drawer()
                    .map(|drawer| drawer.map(Message::DeviceSettingsScreen)),
            }
        }
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::None => (),
            #[cfg(target_os = "linux")]
            Message::TrayCommand(command) => return self.handle_tray_command(command),
            #[cfg(target_os = "linux")]
            Message::CloseToTray => {
                if let Some(id) = self.core.main_window_id() {
                    return window::minimize::<cosmic::Action<Message>>(id, true);
                }
            }
            Message::KeyPressed {
                modifiers,
                key,
                physical_key,
            } => {
                // workaround for mutable borrow of both self and self.screen in match
                enum ActionKind {
                    Global(KeyBindAction),
                    AddDevice(add_device::Action),
                    DeviceSettings(device_settings::Action),
                }
                let action = match &mut self.screen {
                    Screen::AddDevice(add_device) => Some(ActionKind::AddDevice(
                        add_device.on_key_pressed(modifiers, &key, &physical_key),
                    )),
                    Screen::DeviceSettings(device_settings) => Some(ActionKind::DeviceSettings(
                        device_settings.on_key_pressed(modifiers, &key, &physical_key),
                    )),
                    _ => None,
                }
                .or_else(|| {
                    self.key_binds
                        .iter()
                        .find(|(key_bind, _)| {
                            key_bind.matches(modifiers, &key, Some(&physical_key))
                        })
                        .map(|(_, action)| ActionKind::Global(*action))
                });
                match action {
                    Some(ActionKind::Global(action)) => match action {
                        KeyBindAction::Settings => self.toggle_settings(),
                    },
                    Some(ActionKind::AddDevice(action)) => {
                        return self.handle_add_device_action(action);
                    }
                    Some(ActionKind::DeviceSettings(action)) => {
                        return self.handle_device_settings_action(action);
                    }
                    None => (),
                }
            }
            Message::AddDeviceScreen(message) => {
                if let Screen::AddDevice(ref mut screen) = self.screen {
                    match screen.update(message) {
                        add_device::Action::None => (),
                        add_device::Action::Task(task) => {
                            return task.map(Message::AddDeviceScreen).map(Into::into);
                        }
                        add_device::Action::AddDevice(paired_device) => {
                            return self.handle_add_device_action(add_device::Action::AddDevice(
                                paired_device,
                            ));
                        }
                        add_device::Action::FocusTextInput(id) => {
                            return widget::text_input::focus(id);
                        }
                    }
                }
            }
            Message::ActivateConnectToDeviceScreen(device) => {
                let device_arc = device.0.clone();
                let (model, task) = device_settings::DeviceSettingsModel::new(
                    device,
                    self.session.quick_preset_handler(),
                    self.config_dir.to_owned(),
                );
                self.screen = Screen::DeviceSettings(model);

                #[cfg(target_os = "linux")]
                {
                    self.current_device = Some(device_arc.clone());
                    self.tray
                        .update(|tray| tray.set_device(Some(device_arc.clone())));
                    let tray = self.tray.clone();
                    let watch_task = Task::future(async move {
                        let mut watch = device_arc.watch_for_changes();
                        while watch.changed().await.is_ok() {
                            tray.update(|_| {});
                        }
                        Ok(Message::None.into())
                    })
                    .map(coalesce_result);
                    return cosmic::Task::batch([
                        task.map(Message::DeviceSettingsScreen).map(Into::into),
                        watch_task,
                    ]);
                }
                #[cfg(not(target_os = "linux"))]
                return task.map(Message::DeviceSettingsScreen).map(Into::into);
            }
            Message::DeviceSettingsScreen(message) => {
                let maybe_action = if let Screen::DeviceSettings(ref mut screen) = self.screen {
                    Some(screen.update(message))
                } else {
                    None
                };
                if let Some(action) = maybe_action {
                    return self.handle_device_settings_action(action);
                }
            }
            Message::ConnectToDevice(paired_device) => {
                let canceled = Arc::new(Semaphore::new(0));
                self.screen = Screen::Connecting {
                    canceled: canceled.clone(),
                    name: paired_device.model.translate(),
                };
                return connect_to_device(self.session.clone(), paired_device, canceled);
            }
            Message::CancelConnectToDevice => {
                if let Screen::Connecting { canceled, .. } = &self.screen {
                    canceled.close();
                    return Task::done(Message::ShowAddDevice.into());
                }
            }
            Message::ConnectToDeviceFailed(message) => {
                return Task::batch([
                    Task::done(Message::ShowAddDevice.into()),
                    Task::done(Message::Warning(message).into()),
                ]);
            }
            Message::ShowAddDevice => {
                let (model, task) = AddDeviceModel::new(self.session.clone());
                self.screen = Screen::AddDevice(model);
                return task.map(Message::AddDeviceScreen).map(Into::into);
            }
            Message::Warning(message) => {
                // cap max number of warnings, since it's bad UX to have to close a million of them if something goes wrong and spams them
                if self.warnings.capacity() == self.warnings.len() {
                    self.warnings.pop_front();
                }
                self.warnings.push_back(message);
            }
            Message::CloseWarning => {
                self.warnings.pop_front();
            }
            Message::CloseContextDrawer => self.context_drawer_screen = None,
            Message::ToggleAbout => {
                if matches!(self.context_drawer_screen, Some(ContextDrawerScreen::About)) {
                    self.context_drawer_screen = None;
                } else {
                    self.context_drawer_screen = Some(ContextDrawerScreen::About);
                }
            }
            Message::OpenUrl(url) => {
                if let Err(err) = open::that_detached(&url) {
                    tracing::error!("error opening url {url}: {err:?}");
                }
            }
            Message::ToggleSettings => self.toggle_settings(),
            Message::SetPreferredLanguage(language_index) => {
                let result_receiver = self.config.modify(|inner| {
                    inner.preferred_language = self.available_languages[language_index]
                        .as_ref()
                        .map(ToString::to_string);
                });

                return Task::future(async move {
                    if let Err(err) = result_receiver.await.unwrap() {
                        tracing::error!("error writing to config file: {err:?}");
                        Message::Warning(err.to_string())
                    } else {
                        Message::None
                    }
                })
                .map(Into::into);
            }
        }
        Task::none()
    }
}

impl AppModel {
    pub fn update_title(&mut self) -> cosmic::app::Task<Message> {
        if let Some(id) = self.core.main_window_id() {
            self.set_header_title(fl!("openscq30"));
            self.set_window_title(fl!("openscq30"), id)
        } else {
            Task::none()
        }
    }

    fn view_cancel(&self, device_name: &str) -> cosmic::Element<'_, Message> {
        widget::column![
            widget::progress_bar::indeterminate_circular(),
            widget::text::title2(fl!("connecting-to", name = device_name)),
            widget::button::destructive(fl!("cancel")).on_press(Message::CancelConnectToDevice),
        ]
        .spacing(10)
        .align_x(alignment::Horizontal::Center)
        .apply(widget::container)
        .center(Length::Fill)
        .into()
    }

    fn handle_add_device_action(
        &mut self,
        action: add_device::Action,
    ) -> cosmic::app::Task<Message> {
        match action {
            add_device::Action::None => cosmic::app::Task::none(),
            add_device::Action::Task(task) => task.map(Message::AddDeviceScreen).map(Into::into),
            add_device::Action::AddDevice(paired_device) => {
                let database = self.session.clone();
                Task::future(async move {
                    database
                        .pair(paired_device)
                        .await
                        .map_err(handle_soft_error!())?;
                    Ok(Message::ConnectToDevice(paired_device).into())
                })
                .map(coalesce_result)
            }
            add_device::Action::FocusTextInput(id) => widget::text_input::focus(id),
        }
    }

    fn handle_device_settings_action(
        &mut self,
        action: device_settings::Action,
    ) -> cosmic::app::Task<Message> {
        match action {
            device_settings::Action::None => Task::none(),
            device_settings::Action::Task(task) => {
                task.map(Message::DeviceSettingsScreen).map(Into::into)
            }
            device_settings::Action::Warning(message) => {
                Task::done(Message::Warning(message).into())
            }
            device_settings::Action::FocusTextInput(id) => widget::text_input::focus(id),
            device_settings::Action::Disconnect => {
                #[cfg(target_os = "linux")]
                {
                    self.current_device = None;
                    self.tray.update(|tray| tray.set_device(None));
                }
                Task::done(Message::ShowAddDevice.into())
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn handle_tray_command(&mut self, command: TrayCommand) -> cosmic::app::Task<Message> {
        match command {
            TrayCommand::SetAmbientSoundMode(mode) => {
                if let Some(device) = self.current_device.clone() {
                    return Task::future(async move {
                        device
                            .set_setting_values(vec![(
                                SettingId::AmbientSoundMode,
                                Value::String(Cow::Owned(mode)),
                            )])
                            .await
                            .map_err(handle_soft_error!())?;
                        Ok(Message::None.into())
                    })
                    .map(coalesce_result);
                }
                Task::none()
            }
            TrayCommand::OpenSettings => {
                if let Some(id) = self.core.main_window_id() {
                    window::gain_focus::<cosmic::Action<Message>>(id)
                } else {
                    Task::none()
                }
            }
            TrayCommand::Quit => cosmic::iced::exit::<cosmic::Action<Message>>(),
        }
    }

    fn toggle_settings(&mut self) {
        if matches!(
            self.context_drawer_screen,
            Some(ContextDrawerScreen::Settings)
        ) {
            self.context_drawer_screen = None;
        } else {
            self.context_drawer_screen = Some(ContextDrawerScreen::Settings);
        }
    }
}

fn key_binds() -> HashMap<KeyBind, KeyBindAction> {
    let mut key_binds = HashMap::new();

    key_binds.insert(
        KeyBind {
            modifiers: vec![widget::menu::key_bind::Modifier::Ctrl],
            key: keyboard::Key::Character(",".into()),
        },
        KeyBindAction::Settings,
    );

    key_binds
}
