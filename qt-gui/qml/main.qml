import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigami.controls as KC
import org.kde.kirigami.platform as Platform

KC.ApplicationWindow {
    id: root
    visible: true
    width: 780
    height: 620
    minimumWidth: 480
    minimumHeight: 420
    title: "OpenSCQ30"

    Component.onCompleted: Backend.startup()

    // ---- tray integration: hide to tray instead of quitting ----
    onClosing: (close) => {
        close.accepted = false
        root.hide()
    }

    Connections {
        target: Backend
        function onOpenRequested() {
            root.show()
            root.raise()
            root.requestActivate()
        }
    }

    // ---- helper functions ----
    function batteryPercent(value) {
        if (!value)
            return "—"
        var parts = String(value).split("/")
        if (parts.length !== 2)
            return value
        var n = parseInt(parts[0]), d = parseInt(parts[1])
        if (isNaN(n) || isNaN(d) || d === 0)
            return value
        return Math.round(n / d * 100) + "%"
    }
    function batteryColor(value) {
        var p = batteryPercent(value)
        if (p === "—")
            return Platform.Theme.disabledTextColor
        var n = parseInt(p)
        if (isNaN(n))
            return Platform.Theme.disabledTextColor
        if (n >= 30)
            return Platform.Theme.positiveTextColor
        if (n >= 15)
            return Platform.Theme.neutralTextColor
        return Platform.Theme.negativeTextColor
    }

    globalDrawer: KC.GlobalDrawer {
        title: "OpenSCQ30"
        titleIcon: "audio-headphones"
        actions: [
            KC.Action {
                text: "Device"
                icon.name: "audio-headphones"
                onTriggered: root.pageStack.replace(devicePage)
            },
            KC.Action {
                text: "Settings"
                icon.name: "configure"
                enabled: Backend.state == "connected"
                onTriggered: root.pageStack.replace(settingsPage)
            },
            KC.Action {
                text: "Disconnect"
                icon.name: "network-disconnect"
                enabled: Backend.state == "connected"
                onTriggered: Backend.disconnect()
            },
            KC.Action {
                text: "Quit"
                icon.name: "application-exit"
                onTriggered: Backend.quit()
            }
        ]
    }

    pageStack.initialPage: devicePage

    // ---- Device page (dashboard when connected, pairing when not) ----
    Component {
        id: devicePage
        KC.Page {
            title: "OpenSCQ30"

            ColumnLayout {
                anchors.fill: parent
                spacing: Platform.Units.largeSpacing

                // Connected dashboard
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: Platform.Units.largeSpacing
                    visible: Backend.state == "connected"

                    // Hero: icon badge + device name + status
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.largeSpacing

                        Rectangle {
                            width: 56
                            height: 56
                            radius: 28
                            color: Platform.Theme.highlightColor

                            QQC2.Label {
                                anchors.centerIn: parent
                                text: "♫"
                                color: Platform.Theme.highlightedTextColor
                                font.pixelSize: 26
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Platform.Units.smallSpacing

                            KC.Heading {
                                text: Backend.deviceName
                                level: 1
                            }
                            RowLayout {
                                spacing: Platform.Units.smallSpacing

                                Rectangle {
                                    width: 9
                                    height: 9
                                    radius: 4.5
                                    color: Backend.state == "connected" ? Platform.Theme.positiveTextColor : Platform.Theme.disabledTextColor
                                }
                                QQC2.Label {
                                    text: Backend.statusMessage
                                    color: Platform.Theme.disabledTextColor
                                }
                            }
                        }
                    }

                    // Battery
                    KC.Heading {
                        text: "Battery"
                        level: 2
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.largeSpacing

                        QQC2.Frame {
                            Layout.fillWidth: true
                            ColumnLayout {
                                spacing: Platform.Units.smallSpacing

                                QQC2.Label {
                                    text: "Left"
                                    font.bold: true
                                    color: Platform.Theme.disabledTextColor
                                }
                                QQC2.Label {
                                    text: batteryPercent(Backend.batteryLeft)
                                    font.pixelSize: 28
                                    font.bold: true
                                    color: batteryColor(Backend.batteryLeft)
                                }
                                QQC2.Label {
                                    text: Backend.chargingLeft ? "⚡ Charging" : "Battery"
                                    color: Backend.chargingLeft ? Platform.Theme.positiveTextColor : Platform.Theme.disabledTextColor
                                }
                            }
                        }

                        QQC2.Frame {
                            Layout.fillWidth: true
                            ColumnLayout {
                                spacing: Platform.Units.smallSpacing

                                QQC2.Label {
                                    text: "Right"
                                    font.bold: true
                                    color: Platform.Theme.disabledTextColor
                                }
                                QQC2.Label {
                                    text: batteryPercent(Backend.batteryRight)
                                    font.pixelSize: 28
                                    font.bold: true
                                    color: batteryColor(Backend.batteryRight)
                                }
                                QQC2.Label {
                                    text: Backend.chargingRight ? "⚡ Charging" : "Battery"
                                    color: Backend.chargingRight ? Platform.Theme.positiveTextColor : Platform.Theme.disabledTextColor
                                }
                            }
                        }
                    }

                    // Sound mode
                    KC.Heading {
                        text: "Sound Mode"
                        level: 2
                    }
                    QQC2.ButtonGroup {
                        id: ancGroup
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Platform.Units.smallSpacing

                        QQC2.Button {
                            text: "Normal"
                            icon.name: "audio-volume-medium"
                            checkable: true
                            checked: Backend.ancMode == "Normal"
                            QQC2.ButtonGroup.group: ancGroup
                            onClicked: Backend.setAncMode("Normal")
                        }
                        QQC2.Button {
                            text: "Transparency"
                            icon.name: "audio-volume-low"
                            checkable: true
                            checked: Backend.ancMode == "Transparency"
                            QQC2.ButtonGroup.group: ancGroup
                            onClicked: Backend.setAncMode("Transparency")
                        }
                        QQC2.Button {
                            text: "Noise Cancelling"
                            icon.name: "audio-volume-muted"
                            checkable: true
                            checked: Backend.ancMode == "NoiseCanceling"
                            QQC2.ButtonGroup.group: ancGroup
                            onClicked: Backend.setAncMode("NoiseCanceling")
                        }
                    }

                    Item { Layout.fillHeight: true }
                }

                // Connecting indicator
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: Platform.Units.largeSpacing
                    visible: Backend.state == "connecting"
                    Layout.alignment: Qt.AlignCenter

                    QQC2.BusyIndicator {
                        running: true
                        Layout.alignment: Qt.AlignHCenter
                    }
                    QQC2.Label {
                        text: Backend.statusMessage
                        Layout.alignment: Qt.AlignHCenter
                        font.pixelSize: 16
                    }
                }

                // Pairing view (disconnected)
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: Platform.Units.largeSpacing
                    visible: Backend.state == "disconnected"

                    KC.Heading {
                        text: "Connect your Soundcore R50i NC"
                        level: 1
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                    }
                    QQC2.Label {
                        text: "Make sure your earbuds are powered on and Bluetooth is enabled, then pick your device below."
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                    }
                    QQC2.Label {
                        text: Backend.statusMessage
                        color: Platform.Theme.disabledTextColor
                        visible: Backend.statusMessage != ""
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        QQC2.Button {
                            text: "Reconnect"
                            icon.name: "network-connect"
                            onClicked: Backend.startup()
                        }
                        QQC2.Button {
                            text: "Scan for devices"
                            icon.name: "view-refresh"
                            onClicked: Backend.listDevices()
                        }
                        Item { Layout.fillWidth: true }
                    }

                    QQC2.ScrollView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true

                        ColumnLayout {
                            width: parent.width
                            spacing: Platform.Units.smallSpacing

                            Repeater {
                                model: Backend.availableDevices
                                delegate: QQC2.Button {
                                    Layout.fillWidth: true
                                    text: modelData.name + "  (" + modelData.mac + ")"
                                    icon.name: "audio-headphones"
                                    onClicked: Backend.pairAndConnect(modelData.mac, false)
                                }
                            }

                            QQC2.Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                color: Platform.Theme.disabledTextColor
                                text: "No devices found yet. Try scanning again."
                                visible: Backend.availableDevices.length === 0 && !Backend.busy
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- Settings page ----
    Component {
        id: settingsPage
        KC.ScrollablePage {
            title: "Settings"

            ColumnLayout {
                width: parent.width
                spacing: Platform.Units.largeSpacing

                // Sound mode
                KC.Heading {
                    text: "Sound Mode"
                    level: 2
                }
                QQC2.ButtonGroup {
                    id: settingsAncGroup
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: Platform.Units.smallSpacing

                    QQC2.Button {
                        text: "Normal"
                        icon.name: "audio-volume-medium"
                        checkable: true
                        checked: Backend.ancMode == "Normal"
                        QQC2.ButtonGroup.group: settingsAncGroup
                        onClicked: Backend.setAncMode("Normal")
                    }
                    QQC2.Button {
                        text: "Transparency"
                        icon.name: "audio-volume-low"
                        checkable: true
                        checked: Backend.ancMode == "Transparency"
                        QQC2.ButtonGroup.group: settingsAncGroup
                        onClicked: Backend.setAncMode("Transparency")
                    }
                    QQC2.Button {
                        text: "Noise Cancelling"
                        icon.name: "audio-volume-muted"
                        checkable: true
                        checked: Backend.ancMode == "NoiseCanceling"
                        QQC2.ButtonGroup.group: settingsAncGroup
                        onClicked: Backend.setAncMode("NoiseCanceling")
                    }
                }

                // Equalizer
                KC.Heading {
                    text: "Equalizer"
                    level: 2
                }
                QQC2.ComboBox {
                    Layout.fillWidth: true
                    model: Backend.eqPresets
                    currentIndex: Backend.eqPresets.indexOf(Backend.eqPreset)
                    onActivated: (index) => Backend.setSelect("presetEqualizerProfile", Backend.eqPresets[index])
                }
                Repeater {
                    model: Backend.eqBands
                    delegate: RowLayout {
                        width: parent.width
                        spacing: Platform.Units.smallSpacing

                        QQC2.Label {
                            text: (Backend.eqBandHz[index] / 1000).toFixed(1) + " kHz"
                            Layout.minimumWidth: 56
                        }
                        QQC2.Slider {
                            Layout.fillWidth: true
                            from: Backend.eqMin
                            to: Backend.eqMax
                            value: modelData
                            onMoved: Backend.setEqualizerBand("volumeAdjustments", index, value)
                        }
                        QQC2.Label {
                            text: modelData
                            Layout.minimumWidth: 28
                            horizontalAlignment: Text.AlignRight
                        }
                    }
                }

                // Features
                KC.Heading {
                    text: "Features"
                    level: 2
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.Label {
                        text: "Gaming Mode"
                        Layout.fillWidth: true
                    }
                    QQC2.Switch {
                        checked: Backend.gamingMode
                        onToggled: Backend.setToggle("gamingMode", checked)
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.Label {
                        text: "Dual Connections"
                        Layout.fillWidth: true
                    }
                    QQC2.Switch {
                        checked: Backend.dualConnections
                        onToggled: Backend.setToggle("dualConnections", checked)
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.Label {
                        text: "Touch Tone"
                        Layout.fillWidth: true
                    }
                    QQC2.Switch {
                        checked: Backend.touchTone
                        onToggled: Backend.setToggle("touchTone", checked)
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.Label {
                        text: "Low Battery Prompt"
                        Layout.fillWidth: true
                    }
                    QQC2.Switch {
                        checked: Backend.lowBatteryPrompt
                        onToggled: Backend.setToggle("lowBatteryPrompt", checked)
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.Label {
                        text: "Wind Noise Suppression"
                        Layout.fillWidth: true
                    }
                    QQC2.Switch {
                        checked: Backend.windNoiseSuppression
                        onToggled: Backend.setToggle("windNoiseSuppression", checked)
                    }
                }

                // Power
                KC.Heading {
                    text: "Power"
                    level: 2
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.Label {
                        text: "Auto Power Off"
                        Layout.fillWidth: true
                    }
                    QQC2.ComboBox {
                        model: ["10m", "20m", "30m", "60m"]
                        currentIndex: ["10m", "20m", "30m", "60m"].indexOf(Backend.autoPowerOff)
                        onActivated: (index) => Backend.setSelect("autoPowerOff", ["10m", "20m", "30m", "60m"][index])
                    }
                }

                // Device information
                KC.Heading {
                    text: "Device"
                    level: 2
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.Label {
                        text: "Serial Number"
                        Layout.fillWidth: true
                    }
                    QQC2.Label {
                        text: Backend.serialNumber
                        color: Platform.Theme.disabledTextColor
                        textFormat: Text.PlainText
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    QQC2.Label {
                        text: "Firmware"
                        Layout.fillWidth: true
                    }
                    QQC2.Label {
                        text: Backend.firmwareVersion
                        color: Platform.Theme.disabledTextColor
                        textFormat: Text.PlainText
                    }
                }
            }
        }
    }
}
