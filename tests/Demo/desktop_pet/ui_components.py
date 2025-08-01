# file: ui_components.py

from PyQt6.QtWidgets import QWidget
from PyQt6.QtGui import QPixmap, QPainter, QPen, QColor
from PyQt6.QtCore import Qt, pyqtSignal, QPoint, QRect

class ScreenshotOverlay(QWidget):
    screenshot_taken = pyqtSignal(QPixmap)
    finished = pyqtSignal()

    def __init__(self, screen, full_desktop_pixmap):
        super().__init__()
        self.screen = screen
        self.full_desktop_pixmap = full_desktop_pixmap
        
        self.setGeometry(self.screen.geometry())
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.WindowStaysOnTopHint)
        self.setCursor(Qt.CursorShape.CrossCursor)
        self.setMouseTracking(True)
        
        self.begin = QPoint()
        self.end = QPoint()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        monitor_geo = self.screen.geometry()
        ratio = self.screen.devicePixelRatio()
        virtual_desktop_geo = self.screen.virtualGeometry()
        offset_from_virtual_origin = monitor_geo.topLeft() - virtual_desktop_geo.topLeft()
        source_rect = QRect(
            int(offset_from_virtual_origin.x() * ratio),
            int(offset_from_virtual_origin.y() * ratio),
            int(monitor_geo.width() * ratio),
            int(monitor_geo.height() * ratio)
        )
        painter.drawPixmap(self.rect(), self.full_desktop_pixmap, source_rect)
        overlay_color = QColor(0, 0, 0, 120)
        painter.fillRect(self.rect(), overlay_color)
        
        if not self.begin.isNull() and not self.end.isNull():
            selection_rect_local = QRect(self.begin, self.end).normalized()
            selection_offset_from_monitor_origin_logical = selection_rect_local.topLeft()
            total_offset_logical = offset_from_virtual_origin + selection_offset_from_monitor_origin_logical
            selection_source_rect = QRect(
                int(total_offset_logical.x() * ratio),
                int(total_offset_logical.y() * ratio),
                int(selection_rect_local.width() * ratio),
                int(selection_rect_local.height() * ratio)
            )
            painter.drawPixmap(selection_rect_local, self.full_desktop_pixmap, selection_source_rect)
            painter.setPen(QPen(Qt.GlobalColor.white, 1, Qt.PenStyle.DashLine))
            painter.drawRect(selection_rect_local)

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self.begin = event.pos()
            self.end = self.begin
            self.update()

    def mouseMoveEvent(self, event):
        self.end = event.pos()
        self.update()

    def mouseReleaseEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            selection_rect_local = QRect(self.begin, self.end).normalized()
            if selection_rect_local.width() > 5 and selection_rect_local.height() > 5:
                monitor_geo = self.screen.geometry()
                ratio = self.screen.devicePixelRatio()
                virtual_desktop_geo = self.screen.virtualGeometry()
                offset_from_virtual_origin = monitor_geo.topLeft() - virtual_desktop_geo.topLeft()
                selection_offset_from_monitor_origin_logical = selection_rect_local.topLeft()
                total_offset_logical = offset_from_virtual_origin + selection_offset_from_monitor_origin_logical
                final_crop_rect_physical = QRect(
                    int(total_offset_logical.x() * ratio),
                    int(total_offset_logical.y() * ratio),
                    int(selection_rect_local.width() * ratio),
                    int(selection_rect_local.height() * ratio)
                )
                final_pixmap = self.full_desktop_pixmap.copy(final_crop_rect_physical)
                final_pixmap.setDevicePixelRatio(ratio)
                self.screenshot_taken.emit(final_pixmap)
            self.finished.emit()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key.Key_Escape:
            self.finished.emit()