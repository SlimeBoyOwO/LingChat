package api

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"

	"github.com/gorilla/websocket"
)

// Message 表示预期的 JSON 结构
type Message struct {
	Type    string `json:"type"`
	Content string `json:"content"`
}

// Response 表示服务器响应结构
type Response struct {
	Type            string `json:"type" yaml:"type"`
	Emotion         string `json:"emotion" yaml:"emotion"`
	OriginalTag     string `json:"originalTag" yaml:"originalTag"`
	Message         string `json:"message" yaml:"message"`
	MotionText      string `json:"motionText" yaml:"motionText"`
	AudioFile       string `json:"audioFile" yaml:"audioFile"`
	OriginalMessage string `json:"originalMessage" yaml:"originalMessage"`
	IsMultiPart     bool   `json:"isMultiPart" yaml:"isMultiPart"`
	PartIndex       int    `json:"partIndex" yaml:"partIndex"`
	TotalParts      int    `json:"totalParts" yaml:"totalParts"`
	Error           string `json:"error,omitempty"`
}

type Sentence []byte

// MessageHandler 定义消息处理接口
type MessageHandler func([]byte) ([]Sentence, error)

var upgrader = websocket.Upgrader{
	ReadBufferSize:  1024,
	WriteBufferSize: 1024,
	CheckOrigin:     func(r *http.Request) bool { return true }, // 允许所有来源
}

var TestHandler MessageHandler = func(rawMsg []byte) ([]Sentence, error) {
	var msg Message
	err := json.Unmarshal(rawMsg, &msg)
	if err != nil {
		err = fmt.Errorf("JSON 解析错误: %w", err)
		log.Println(err)
		return nil, err
	}

	// // 使用注入的处理器处理消息
	// response := []byte{}
	// if err != nil {
	// 	err = fmt.Errorf("处理错误: %w", err)
	// 	log.Println(err)
	// 	return nil, err
	// }
	fmt.Println(msg)

	responseJSON, err := json.Marshal(msg)
	if err != nil {
		err = fmt.Errorf("JSON 序列化错误: %w", err)
		log.Println(err)
		return nil, err
	}
	return []Sentence{responseJSON}, nil
}

// WebSocketHandler 管理 WebSocket 连接
type WebSocketHandler struct {
	handler MessageHandler
}

// NewWebSocketHandler 创建新的 WebSocket 服务器
func NewWebSocketHandler(handler MessageHandler) *WebSocketHandler {
	return &WebSocketHandler{
		handler: handler,
	}
}

func (s *WebSocketHandler) HandleWebSocket(w http.ResponseWriter, r *http.Request) {
	// 将 HTTP 连接升级为 WebSocket
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("WebSocket升级错误: %v", err)
		return
	}
	defer conn.Close()

	log.Printf("新的WebSocket连接已建立: %s", r.RemoteAddr)

	for {
		// 读取消息
		_, rawMessage, err := conn.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				log.Printf("WebSocket连接异常关闭: %v", err)
			} else {
				log.Println("读取错误:", err)
			}
			break
		}

		// 处理消息
		rawResp, err := s.handler(rawMessage)
		if err != nil {
			log.Printf("消息处理错误: %v", err)
			errorResp := Response{
				Type:  "error",
				Error: err.Error(),
			}
			errorJSON, _ := json.Marshal(errorResp)
			if err := conn.WriteMessage(websocket.TextMessage, errorJSON); err != nil {
				log.Printf("发送错误响应失败: %v", err)
				break
			}
			continue
		}

		// 发送响应
		for _, msg := range rawResp {
			if err := conn.WriteMessage(websocket.TextMessage, msg); err != nil {
				log.Printf("发送响应失败: %v", err)
				break
			}
		}
	}

	log.Printf("WebSocket连接已关闭: %s", r.RemoteAddr)
}
