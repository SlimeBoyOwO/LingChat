package service

import (
	"context"
	"fmt"
	"log"
	"os"
	"testing"

	"github.com/joho/godotenv"

	"LingChat/api"
	"LingChat/internal/clients/VitsTTS"
	"LingChat/internal/clients/emotionPredictor"
	"LingChat/internal/clients/llm"
	"LingChat/internal/config"
)

var conf *config.Config
var service *LingChatService
var ctx = context.Background()

func init() {
	fmt.Println(os.Getwd())
	err := godotenv.Load()
	if err != nil {
		log.Fatal("无法加载 .env 文件: ", err)
	}
	conf = config.GetConfigFromEnv()
	fmt.Println(conf.Chat.BaseURL)

	emotionPredictorClient := emotionPredictor.NewClient(conf.Emotion.URL)
	vitsTTSClient := VitsTTS.NewClient(conf.Vits.APIURL, conf.TempDirs.VoiceDir)
	llmClient := llm.NewLLMClient(conf.Chat.BaseURL, conf.Chat.APIKey)

	service = NewLingChatService(emotionPredictorClient, vitsTTSClient, llmClient, conf.TempDirs.VoiceDir)
}

func Test_ChatAndParse(t *testing.T) {
	rawResp, err := service.llmClient.Chat(ctx, "你好")
	if err != nil {
		t.Fatal(err)
	}
	fmt.Println(AnalyzeEmotions(rawResp, service.tempFilePath, "wav"))
}

func Test_LingChat(t *testing.T) {
	fmt.Println(service.LingChat(ctx, api.Message{
		Type:    "message",
		Content: "你好",
	}))
}
