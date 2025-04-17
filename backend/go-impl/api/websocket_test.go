package api

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

func TestWebSocketServer(t *testing.T) {
	wsServer := NewWebSocketHandler(TestHandler)
	// 创建测试服务器
	server := httptest.NewServer(http.HandlerFunc(wsServer.HandleWebSocket))
	defer server.Close()

	// 将 http:// 替换为 ws://
	wsURL := "ws" + strings.TrimPrefix(server.URL, "http") + "/ws"

	// 连接到 WebSocket 服务器
	ws, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("无法连接到 WebSocket 服务器: %v", err)
	}
	defer ws.Close()

	// 准备测试用例
	testCases := []struct {
		name     string
		message  Message
		expected Message
	}{
		{
			name:     "标准消息",
			message:  Message{Type: "message", Content: "hi"},
			expected: Message{Type: "message", Content: "hi"},
		},
		{
			name:     "其他类型消息",
			message:  Message{Type: "notification", Content: "test"},
			expected: Message{Type: "notification", Content: "test"},
		},
	}

	// 执行测试用例
	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			// 发送消息
			messageData, err := json.Marshal(tc.message)
			if err != nil {
				t.Fatalf("JSON 编码错误: %v", err)
			}

			err = ws.WriteMessage(websocket.TextMessage, messageData)
			if err != nil {
				t.Fatalf("发送消息错误: %v", err)
			}

			// 设置读取超时
			ws.SetReadDeadline(time.Now().Add(time.Second))

			// 读取响应
			_, response, err := ws.ReadMessage()
			if err != nil {
				t.Fatalf("读取响应错误: %v", err)
			}

			// 解析响应
			var receivedMsg Message
			if err := json.Unmarshal(response, &receivedMsg); err != nil {
				t.Fatalf("JSON 解析错误: %v", err)
			}

			// 验证响应
			if receivedMsg.Type != tc.expected.Type || receivedMsg.Content != tc.expected.Content {
				t.Errorf("响应不匹配。期望: %+v, 实际: %+v", tc.expected, receivedMsg)
			}
		})
	}
}
