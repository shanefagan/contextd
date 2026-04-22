import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: window
    width: 800
    height: 600
    visible: true
    title: "Contextd RGB Vibe Tester"
    color: "#0d0d12" // Deep dark background

    // The ambient glowing background (glow effect based on ambient color)
    Rectangle {
        id: ambientGlow
        anchors.fill: parent
        color: rgbModel.mainColor
        opacity: 0.15 // Subtle glow
        
        Behavior on color {
            ColorAnimation { duration: 500; easing.type: Easing.InOutQuad }
        }
    }
    
    // Main Content
    Item {
        anchors.fill: parent
        anchors.margins: 40

        ColumnLayout {
            anchors.fill: parent
            spacing: 20

            // Header Section
            ColumnLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 10

                Text {
                    text: "Lighting Vibe Viewer"
                    color: "#ffffff"
                    font.pixelSize: 32
                    font.weight: Font.Bold
                    font.family: "Inter, Roboto, sans-serif"
                    Layout.alignment: Qt.AlignHCenter
                }

                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: 15

                    Text {
                        text: "Ambient Vibe:"
                        color: "#a0a0ab"
                        font.pixelSize: 16
                        font.family: "Inter, Roboto, sans-serif"
                    }

                    // Ambient Color Bubble
                    Rectangle {
                        width: 32
                        height: 32
                        radius: 16
                        color: rgbModel.mainColor
                        border.color: "#44ffffff"
                        border.width: 1

                        Behavior on color {
                            ColorAnimation { duration: 500; easing.type: Easing.InOutQuad }
                        }
                    }
                }
            }

            // Matrix Section (Glassy Card)
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.margins: 20
                color: "#181820"
                radius: 24
                border.color: "#2a2a35"
                border.width: 2
                clip: true

                // Matrix Grid
                GridLayout {
                    anchors.centerIn: parent
                    anchors.margins: 20
                    columns: rgbModel.matrixWidth > 0 ? rgbModel.matrixWidth : 1
                    
                    // Scale spacing down for large grids based on total pixels
                    property int totalPixels: rgbModel.matrixWidth * rgbModel.matrixHeight
                    property real dynamicSpacing: totalPixels > 1024 ? 0 : (totalPixels > 256 ? 1 : 4)
                    rowSpacing: dynamicSpacing
                    columnSpacing: dynamicSpacing
                    
                    Text {
                        visible: rgbModel.matrixWidth === 0
                        text: "No matrix data received yet"
                        color: "#666677"
                        font.pixelSize: 16
                        font.italic: true
                        font.family: "Inter, Roboto, sans-serif"
                        Layout.alignment: Qt.AlignCenter
                    }

                    Repeater {
                        model: rgbModel.matrixData
                        
                        Rectangle {
                            property color cellColor: modelData
                            
                            // Dynamic sizing to fit the container
                            Layout.preferredWidth: {
                                if (rgbModel.matrixWidth === 0 || rgbModel.matrixHeight === 0) return 0;
                                let availableW = parent.parent.width - 40;
                                let availableH = parent.parent.height - 40;
                                
                                let spacingSumW = (rgbModel.matrixWidth - 1) * parent.dynamicSpacing;
                                let spacingSumH = (rgbModel.matrixHeight - 1) * parent.dynamicSpacing;
                                
                                let w = (availableW - spacingSumW) / rgbModel.matrixWidth;
                                let h = (availableH - spacingSumH) / rgbModel.matrixHeight;
                                
                                // To keep cells square, we take the minimum of max allowed width and max allowed height
                                return Math.max(1, Math.min(w, h)); 
                            }
                            Layout.preferredHeight: Layout.preferredWidth
                            
                            radius: parent.totalPixels > 256 ? 0 : Math.max(4, Layout.preferredWidth * 0.15)
                            color: cellColor
                            
                            Behavior on color {
                                enabled: parent.totalPixels <= 256
                                ColorAnimation { duration: 300; easing.type: Easing.OutCubic }
                            }
                            
                            // Interactive hover micro-animation only for smaller grids
                            MouseArea {
                                anchors.fill: parent
                                hoverEnabled: true
                                enabled: parent.totalPixels <= 256
                                onEntered: parent.scale = 1.15
                                onExited: parent.scale = 1.0
                            }
                            
                            Behavior on scale {
                                enabled: parent.totalPixels <= 256
                                NumberAnimation { duration: 200; easing.type: Easing.OutBack }
                            }
                        }
                    }
                }
            }
        }
    }
}
