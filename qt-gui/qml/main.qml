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

    // ---- helper functions for select settings ----
    function selectIndex(s) {
        if (s.nullable)
            return s.value === "" ? 0 : s.options.indexOf(s.value) + 1
        return s.options.indexOf(s.value)
    }
    function selectLabels(s) {
        return s.nullable ? ["None"].concat(s.labels) : s.labels
    }
    function selectValueAt(s, idx) {
        if (s.nullable)
            return idx === 0 ? "" : s.options[idx - 1]
        return s.options[idx]
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
                enabled: Backend.state === "connected"
                onTriggered: root.pageStack.replace(settingsPage)
            },
            KC.Action {
                text: "Disconnect"
                icon.name: "network-disconnect"
                enabled: Backend.state === "connected"
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

    // ---- One setting row (declared before the pages that use it) ----
    component SettingDelegate: Column {
        required property var setting

        // Label + inline control
        RowLayout {
            width: parent.width
            spacing: Platform.Units.largeSpacing

            QQC2.Label {
                text: setting.label
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            // Toggle
            QQC2.Switch {
                visible: setting.kind === "toggle"
                checked: setting.value === true
                onToggled: Backend.setToggle(setting.id, checked)
            }

            // Select
            QQC2.ComboBox {
                visible: setting.kind === "select"
                Layout.minimumWidth: 180
                model: root.selectLabels(setting)
                currentIndex: root.selectIndex(setting)
                onActivated: (index) => Backend.setSelect(setting.id, root.selectValueAt(setting, index))
            }

            // Range
            QQC2.Slider {
                visible: setting.kind === "range"
                Layout.minimumWidth: 180
                from: setting.min
                to: setting.max
                stepSize: setting.step
                value: setting.value
                onMoved: Backend.setRange(setting.id, value)
            }
            QQC2.Label {
                visible: setting.kind === "range"
                text: setting.value
                Layout.minimumWidth: 24
                horizontalAlignment: Text.AlignRight
            }

            // Action button
            QQC2.Button {
                visible: setting.kind === "action"
                text: "Apply"
                onClicked: Backend.triggerAction(setting.id)
            }

            // Information (read-only)
            QQC2.Label {
                visible: setting.kind === "information"
                text: setting.value
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                textFormat: Text.PlainText
                horizontalAlignment: Text.AlignRight
            }
        }

        // Equalizer bands
        ColumnLayout {
            width: parent.width
            visible: setting.kind === "equalizer"
            spacing: Platform.Units.smallSpacing

            Repeater {
                model: setting.values
                delegate: RowLayout {
                    width: parent.width
                    spacing: Platform.Units.smallSpacing

                    QQC2.Label {
                        text: (setting.bands[index] / 1000).toFixed(1) + " kHz"
                        Layout.minimumWidth: 56
                    }
                    QQC2.Slider {
                        Layout.fillWidth: true
                        from: setting.min
                        to: setting.max
                        value: modelData
                        onMoved: Backend.setEqualizerBand(setting.id, index, value)
                    }
                    QQC2.Label {
                        text: modelData
                        Layout.minimumWidth: 28
                        horizontalAlignment: Text.AlignRight
                    }
                }
            }
        }

        Rectangle {
            width: parent.width
            height: 1
            color: Platform.Theme.disabledTextColor
            opacity: 0.25
        }
    }

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
                    visible: Backend.state === "connected"

                    KC.Heading {
                        text: Backend.deviceName
                        level: 1
                        Layout.fillWidth: true
                    }
                    QQC2.Label {
                        text: Backend.statusMessage
                        color: Platform.Theme.disabledTextColor
                        visible: Backend.statusMessage !== ""
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
                                QQC2.Label {
                                    text: "Left"
                                    font.bold: true
                                }
                                QQC2.Label {
                                    text: Backend.batteryLeft !== "" ? Backend.batteryLeft : "—"
                                    font.pixelSize: 22
                                }
                                QQC2.Label {
                                    text: Backend.chargingLeft ? "Charging" : ""
                                    color: Platform.Theme.positiveTextColor
                                    visible: Backend.chargingLeft
                                }
                            }
                        }

                        QQC2.Frame {
                            Layout.fillWidth: true
                            ColumnLayout {
                                QQC2.Label {
                                    text: "Right"
                                    font.bold: true
                                }
                                QQC2.Label {
                                    text: Backend.batteryRight !== "" ? Backend.batteryRight : "—"
                                    font.pixelSize: 22
                                }
                                QQC2.Label {
                                    text: Backend.chargingRight ? "Charging" : ""
                                    color: Platform.Theme.positiveTextColor
                                    visible: Backend.chargingRight
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
                            checkable: true
                            checked: Backend.ancMode === "Normal"
                            QQC2.ButtonGroup.group: ancGroup
                            onClicked: Backend.setAncMode("Normal")
                        }
                        QQC2.Button {
                            text: "Transparency"
                            checkable: true
                            checked: Backend.ancMode === "Transparency"
                            QQC2.ButtonGroup.group: ancGroup
                            onClicked: Backend.setAncMode("Transparency")
                        }
                        QQC2.Button {
                            text: "Noise Cancelling"
                            checkable: true
                            checked: Backend.ancMode === "NoiseCanceling"
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
                    visible: Backend.state === "connecting"
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
                    visible: Backend.state === "disconnected"

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
                        visible: Backend.statusMessage !== ""
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

                QQC2.ComboBox {
                    id: categoryCombo
                    Layout.fillWidth: true
                    textRole: "label"
                    model: Backend.categories
                    currentIndex: {
                        for (let i = 0; i < Backend.categories.length; ++i) {
                            if (Backend.categories[i].id === Backend.currentCategory)
                                return i
                        }
                        return -1
                    }
                    onActivated: (index) => Backend.setCategory(Backend.categories[index].id)
                }

                Repeater {
                    model: Backend.settings
                    delegate: SettingDelegate {
                        setting: modelData
                        width: parent.width
                    }
                }
            }
        }
    }
}
