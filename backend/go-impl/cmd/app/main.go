package main

import (
	"fmt"
	"log"
	"net/http"

	"github.com/joho/godotenv"
	
	"LingChat/api"
	"LingChat/internal/clients/VitsTTS"
	"LingChat/internal/clients/emotionPredictor"
	"LingChat/internal/clients/llm"
	"LingChat/internal/config"
	"LingChat/internal/service"
)

func main() {
	err := godotenv.Load()
	if err != nil {
		log.Fatal("无法加载 .env 文件: ", err)
	}
	conf := config.GetConfigFromEnv()

	emotionPredictorClient := emotionPredictor.NewClient(conf.Emotion.URL)
	vitsTTSClient := VitsTTS.NewClient(conf.Vits.APIURL, conf.TempDirs.VoiceDir, conf.Vits.SpeakerID)
	llmClient := llm.NewLLMClient(conf.Chat.BaseURL, conf.Chat.APIKey)

	chatService := service.NewLingChatService(emotionPredictorClient, vitsTTSClient, llmClient, conf.TempDirs.VoiceDir)

	// 创建WebSocket服务器
	wsServer := api.NewWebSocketHandler(chatService.ChatHandler)

	// 设置路由
	http.HandleFunc("/", wsServer.HandleWebSocket)

	// 构建服务器地址
	serverAddr := fmt.Sprintf("%s:%d", conf.Backend.BindAddr, conf.Backend.Port)

	// 启动服务器
	log.Printf("WebSocket服务器启动在 %s", serverAddr)
	if err := http.ListenAndServe(serverAddr, nil); err != nil {
		log.Fatal("服务器启动失败: ", err)
	}
}
