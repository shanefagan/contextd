import sys
import json
import socket
from PyQt6.QtCore import QObject, pyqtSignal, pyqtProperty, pyqtSlot, QThread
from PyQt6.QtGui import QGuiApplication, QColor
from PyQt6.QtQml import QQmlApplicationEngine

class VarlinkThread(QThread):
    updateReceived = pyqtSignal(dict)
    
    def run(self):
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.connect("/run/contextd/contextd-rgb-observer.socket")
            
            req = {
                "method": "com.performativenonsense.contextd.rgb.Observer.SubscribeLightingContext",
                "more": True
            }
            sock.sendall(json.dumps(req).encode('utf-8') + b'\0')
            
            buffer = b''
            while True:
                data = sock.recv(4096)
                if not data:
                    print("Socket disconnected.")
                    break
                buffer += data
                
                while b'\0' in buffer:
                    msg_bytes, buffer = buffer.split(b'\0', 1)
                    if msg_bytes:
                        try:
                            msg = json.loads(msg_bytes.decode('utf-8'))
                            if 'parameters' in msg:
                                self.updateReceived.emit(msg['parameters'])
                        except json.JSONDecodeError as e:
                            print(f"Failed to parse JSON: {e}")
        except Exception as e:
            print(f"Varlink thread error: {e}")

class RgbModel(QObject):
    mainColorChanged = pyqtSignal()
    matrixChanged = pyqtSignal()

    def __init__(self):
        super().__init__()
        self._main_color = QColor(50, 50, 50, 255)
        self._matrix_width = 0
        self._matrix_height = 0
        self._matrix_data = []

    @pyqtProperty(QColor, notify=mainColorChanged)
    def mainColor(self):
        return self._main_color

    @pyqtProperty(int, notify=matrixChanged)
    def matrixWidth(self):
        return self._matrix_width

    @pyqtProperty(int, notify=matrixChanged)
    def matrixHeight(self):
        return self._matrix_height

    @pyqtProperty('QVariantList', notify=matrixChanged)
    def matrixData(self):
        return self._matrix_data

    @pyqtSlot(dict)
    def on_update_received(self, params):
        if 'main_color' in params:
            c = params['main_color']
            self._main_color = QColor(c.get('r', 0), c.get('g', 0), c.get('b', 0), c.get('a', 255))
            self.mainColorChanged.emit()
        
        if 'matrix' in params and params['matrix']:
            m = params['matrix']
            self._matrix_width = m.get('width', 0)
            self._matrix_height = m.get('height', 0)
            data = m.get('data', [])
            self._matrix_data = [QColor(c.get('r',0), c.get('g',0), c.get('b',0), c.get('a',255)) for c in data]
            self.matrixChanged.emit()
        else:
            self._matrix_width = 0
            self._matrix_height = 0
            self._matrix_data = []
            self.matrixChanged.emit()

if __name__ == '__main__':
    app = QGuiApplication(sys.argv)
    engine = QQmlApplicationEngine()
    
    model = RgbModel()
    engine.rootContext().setContextProperty("rgbModel", model)
    
    # Load QML from the same directory as the script
    import os
    qml_path = os.path.join(os.path.dirname(__file__), "main.qml")
    engine.load(qml_path)
    
    if not engine.rootObjects():
        sys.exit(-1)
        
    thread = VarlinkThread()
    thread.updateReceived.connect(model.on_update_received)
    thread.start()
    
    sys.exit(app.exec())
