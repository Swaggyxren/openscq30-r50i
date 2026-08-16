use std::{path::Path, process::Command};

use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
use tempfile::tempdir;

fn cli(dir: &Path) -> Command {
    let mut cmd = Command::new(get_cargo_bin("openscq30"));
    cmd.env("XDG_CONFIG_HOME", dir.to_str().unwrap());
    cmd
}

fn add_device(dir: &Path, model: &str) {
    let output = cli(dir)
        .arg("paired-devices")
        .arg("add")
        .arg("--mac-address")
        .arg("00:00:00:00:00:00")
        .arg("--model")
        .arg(model)
        .arg("--demo")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn list_settings_no_extended_info() {
    let dir = tempdir().unwrap();
    add_device(dir.path(), "SoundcoreA3959");
    assert_cmd_snapshot!(cli(dir.path()).arg("device").arg("--mac-address").arg("00:00:00:00:00:00").arg("list-settings").arg("--no-extended-info"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    -- soundModes --
    ambientSoundMode
    noiseCancelingMode
    adaptiveNoiseCanceling
    manualNoiseCanceling
    windNoiseSuppression
    windNoiseDetected
    adaptiveNoiseCancelingSensitivityLevel
    multiSceneNoiseCanceling
    -- equalizer --
    presetEqualizerProfile
    customEqualizerProfile
    volumeAdjustments
    -- equalizerImportExport --
    importCustomEqualizerProfiles
    exportCustomEqualizerProfiles
    exportCustomEqualizerProfilesOutput
    -- buttonConfiguration --
    leftSinglePress
    rightSinglePress
    leftDoublePress
    rightDoublePress
    leftTriplePress
    rightTriplePress
    leftLongPress
    rightLongPress
    normalModeInCycle
    transparencyModeInCycle
    noiseCancelingModeInCycle
    resetButtonsToDefault
    -- dualConnections --
    dualConnections
    dualConnectionsDevices
    -- miscellaneous --
    autoPowerOff
    touchTone
    lowBatteryPrompt
    -- deviceInformation --
    twsStatus
    hostDevice
    isChargingLeft
    isChargingRight
    batteryLevelLeft
    batteryLevelRight
    serialNumber
    firmwareVersionLeft
    firmwareVersionRight

    ----- stderr -----
    ");
}

#[test]
fn list_settings_no_categories_and_no_extended_info() {
    let dir = tempdir().unwrap();
    add_device(dir.path(), "SoundcoreA3959");
    assert_cmd_snapshot!(cli(dir.path()).arg("device").arg("--mac-address").arg("00:00:00:00:00:00").arg("list-settings").arg("--no-categories").arg("--no-extended-info"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    ambientSoundMode
    noiseCancelingMode
    adaptiveNoiseCanceling
    manualNoiseCanceling
    windNoiseSuppression
    windNoiseDetected
    adaptiveNoiseCancelingSensitivityLevel
    multiSceneNoiseCanceling
    presetEqualizerProfile
    customEqualizerProfile
    volumeAdjustments
    importCustomEqualizerProfiles
    exportCustomEqualizerProfiles
    exportCustomEqualizerProfilesOutput
    leftSinglePress
    rightSinglePress
    leftDoublePress
    rightDoublePress
    leftTriplePress
    rightTriplePress
    leftLongPress
    rightLongPress
    normalModeInCycle
    transparencyModeInCycle
    noiseCancelingModeInCycle
    resetButtonsToDefault
    dualConnections
    dualConnectionsDevices
    autoPowerOff
    touchTone
    lowBatteryPrompt
    twsStatus
    hostDevice
    isChargingLeft
    isChargingRight
    batteryLevelLeft
    batteryLevelRight
    serialNumber
    firmwareVersionLeft
    firmwareVersionRight

    ----- stderr -----
    ");
}

#[test]
fn set_and_get_ambient_sound_mode() {
    let dir = tempdir().unwrap();
    add_device(dir.path(), "SoundcoreA3959");
    assert_cmd_snapshot!(cli(dir.path()).arg("device").arg("--mac-address").arg("00:00:00:00:00:00").arg("setting").arg("--set").arg("ambientSoundMode=NoiseCanceling").arg("--get").arg("ambientSoundMode"), @r"
    success: true
    exit_code: 0
    ----- stdout -----
    Setting ID      	Value         
    ambientSoundMode	NoiseCanceling

    ----- stderr -----
    ");
}
