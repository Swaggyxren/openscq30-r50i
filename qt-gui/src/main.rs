use std::{cell::RefCell, sync::Arc};

use qmetaobject::*;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

mod backend;
mod tray;

use backend::Backend;
use tray::TrayCommand;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    // Route QML/Qt log messages (console.log, property binding warnings, …)
    // through the `log` facade into tracing so they reach stderr instead of
    // being silently dropped.
    let _ = tracing_log::LogTracer::init();
    qmetaobject::log::init_qt_to_rust();

    // Create the Qt application. Must be on the main thread and before any QObject.
    let mut engine = QmlEngine::new();

    qmetaobject::qtcore::core_application::QCoreApplication::set_application_name(
        "OpenSCQ30".into(),
    );
    qmetaobject::qtcore::core_application::QCoreApplication::set_organization_name(
        "OpenSCQ30".into(),
    );
    qmetaobject::qtcore::core_application::QCoreApplication::set_application_version(
        env!("CARGO_PKG_VERSION").into(),
    );

    // Async runtime + session. The session talks to the device over tokio in the background.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to start the async runtime");
    let config_dir = dirs::config_dir()
        .expect("failed to find config dir")
        .join("openscq30");
    let _ = std::fs::create_dir_all(&config_dir);
    let session = runtime
        .block_on(openscq30_lib::OpenSCQ30Session::new(
            config_dir.join("database.sqlite"),
        ))
        .expect("database is required to run");
    let session = Arc::new(session);

    // Make the app icon resolvable by name (taskbar / launcher / tooltips).
    install_app_icon();

    // System tray (StatusNotifierItem) on its own thread.
    let (tray_command_tx, tray_command_rx) = tokio::sync::mpsc::unbounded_channel::<TrayCommand>();
    let tray = tray::spawn(tray_command_tx);

    // The QML-facing controller.
    let backend = Backend::new(session, runtime.handle().clone(), tray);

    // Leak the backend so the C++ object stays alive for the whole app.
    let backend_cell: &'static RefCell<Backend> = Box::leak(Box::new(RefCell::new(backend)));
    let pinned = unsafe { QObjectPinned::new(backend_cell) };
    engine.set_object_property("Backend".into(), pinned);

    // Forward tray commands (arriving on the async runtime) onto the Qt thread.
    let backend_qptr = QPointer::from(&*backend_cell.borrow());
    let tray_callback = queued_callback(move |command: TrayCommand| {
        if let Some(pinned) = backend_qptr.as_pinned() {
            pinned.borrow_mut().handle_tray_command(command);
        }
    });
    runtime.spawn(async move {
        let mut rx = tray_command_rx;
        while let Some(command) = rx.recv().await {
            tray_callback(command);
        }
    });

    engine.load_data(include_str!("../qml/main.qml").into());
    engine.exec();
}

/// Writes the app SVG into the user's icon theme so `icon_name` resolves.
fn install_app_icon() {
    let Some(data_dir) = dirs::data_dir() else {
        return;
    };
    let path = data_dir.join("icons/hicolor/scalable/apps/com.oppzippy.OpenSCQ30.svg");
    let svg = include_str!("../resources/com.oppzippy.OpenSCQ30.svg");
    if std::fs::read_to_string(&path).is_ok_and(|existing| existing == svg) {
        return;
    }
    let Some(parent) = path.parent() else {
        return;
    };
    if let Err(err) = std::fs::create_dir_all(parent) {
        tracing::debug!("failed to create icon directory {parent:?}: {err}");
        return;
    }
    if let Err(err) = std::fs::write(&path, svg) {
        tracing::debug!("failed to install app icon to {path:?}: {err}");
    }
}
