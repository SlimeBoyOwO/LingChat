package main

import (
	"context"
	"encoding/base64"
	"fmt"
	"log"
	"net/http"
	"time"

	"github.com/joho/godotenv"

	"LingChat/api"
	"LingChat/api/routes"
	v1 "LingChat/api/routes/v1"
	"LingChat/internal/clients/VitsTTS"
	"LingChat/internal/clients/emotionPredictor"
	"LingChat/internal/clients/llm"
	"LingChat/internal/config"
	"LingChat/internal/data"
	"LingChat/internal/service"
	"LingChat/pkg/jwt"
)

func main() {

	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	defer cancel()

	// init Config
	err := godotenv.Load()
	if err != nil {
		log.Fatal("无法加载 .env 文件: ", err)
	}
	conf := config.GetConfigFromEnv()

	// init pkg instances
	secBytes, err := base64.StdEncoding.DecodeString(conf.Server.JWTSecret)
	if err != nil {
		log.Fatal("decode jwt secret failed: ", err)
	}
	j := jwt.NewJWT(secBytes, "LingChat-Backend")

	// init Clients
	emotionPredictorClient := emotionPredictor.NewClient(conf.Emotion.URL)
	vitsTTSClient := VitsTTS.NewClient(conf.Vits.APIURL, conf.TempDirs.VoiceDir, conf.Vits.SpeakerID)
	llmClient := llm.NewLLMClient(conf.Chat.BaseURL, conf.Chat.APIKey)

	// init Data & Repos
	entClient, err := data.NewEntClient(ctx, conf.Data.DataBase.Driver, conf.Data.DataBase.Source, conf.Data.DataBase.AutoMigrate)
	if err != nil {
		log.Fatal(err)
	}
	d, cleanup, err := data.NewData(entClient, nil)
	defer cleanup()
	if err != nil {
		log.Fatal(err)
	}
	userRepo := data.NewUserRepo(d)

	// init Service
	chatService := service.NewLingChatService(emotionPredictorClient, vitsTTSClient, llmClient, conf.Chat.Model, conf.TempDirs.VoiceDir)
	userService := service.NewUserService(userRepo, j)

	// init HTTP server
	chatRoute := v1.NewChatRoute(chatService, userRepo, j)
	userRoute := v1.NewUserRoute(userService)
	httpEngine := routes.NewHTTPEngine(conf.Backend.BindAddr+":9876", chatRoute, userRoute)
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
