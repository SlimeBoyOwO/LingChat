package main

import (
	"fmt"
	"log"
	"net/http"

	"github.com/joho/godotenv"

	"LingChat/api"
	"LingChat/api/routes"
	v1 "LingChat/api/routes/v1"
	"LingChat/internal/clients/VitsTTS"
	"LingChat/internal/clients/emotionPredictor"
	"LingChat/internal/clients/llm"
	"LingChat/internal/config"
	"LingChat/internal/service"
)

func main() {

	// init Config
	err := godotenv.Load()
	if err != nil {
		log.Fatal("无法加载 .env 文件: ", err)
	}
	conf := config.GetConfigFromEnv()

	// init Clients
	emotionPredictorClient := emotionPredictor.NewClient(conf.Emotion.URL)
	vitsTTSClient := VitsTTS.NewClient(conf.Vits.APIURL, conf.TempDirs.VoiceDir, conf.Vits.SpeakerID)
	llmClient := llm.NewLLMClient(conf.Chat.BaseURL, conf.Chat.APIKey)

	// init Service
	chatService := service.NewLingChatService(emotionPredictorClient, vitsTTSClient, llmClient, conf.Chat.Model, conf.TempDirs.VoiceDir)

	chatRoute := v1.NewChatRoute(chatService)
	httpEngine := routes.NewHTTPEngine(conf.Backend.BindAddr+":9876", chatRoute)
	_, err = httpEngine.Run()
	if err != nil {
		log.Fatal(err)
	}

	// 创建WebSocket服务器
	wsServer := api.NewWebSocketHandler(chatService.ChatHandler)

	// 设置路由
	http.HandleFunc("/", wsServer.HandleWebSocket)

	// 启动服务器
	serverAddr := fmt.Sprintf("%s:%d", conf.Backend.BindAddr, conf.Backend.Port)
	log.Printf("WebSocket服务器启动在 %s", serverAddr)
	if err := http.ListenAndServe(serverAddr, nil); err != nil {
		log.Fatal("服务器启动失败: ", err)
	}
}
