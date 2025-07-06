package llm

import (
	"context"
	"errors"
	"io"
	"log"

	"github.com/sashabaranov/go-openai"
)

type LLMClient struct {
	client  *openai.Client
	apiKey  string
	BaseURL string
}

func NewLLMClient(baseURL, apiKey string) *LLMClient {
	clientConfig := openai.DefaultConfig(apiKey)
	clientConfig.BaseURL = baseURL
	return &LLMClient{
		client:  openai.NewClientWithConfig(clientConfig),
		apiKey:  apiKey,
		BaseURL: baseURL,
	}
}

func (l *LLMClient) Chat(ctx context.Context, messages []openai.ChatCompletionMessage, model string) (string, error) {
	// 创建聊天完成请求
	resp, err := l.client.CreateChatCompletion(
		ctx,
		openai.ChatCompletionRequest{
			Model:    model,
			Messages: messages,
		},
	)

	if err != nil {
		err = errors.Join(errors.New("ChatCompletion error"), err)
		log.Println(err)
		return "", err
	}

	return resp.Choices[0].Message.Content, nil
}

func (l *LLMClient) ChatStream(ctx context.Context, messages []openai.ChatCompletionMessage, model string) (<-chan string, error) {
	// 创建流式聊天请求
	stream, err := l.client.CreateChatCompletionStream(
		ctx,
		openai.ChatCompletionRequest{
			Model:    model,
			Messages: messages,
		},
	)
	if err != nil {
		return nil, errors.Join(errors.New("ChatCompletionStream error"), err)
	}

	// 创建通道用于返回流式结果
	ch := make(chan string)

	// 启动goroutine处理流式响应
	go func() {
		defer close(ch)
		defer stream.Close()

		for {
			response, err := stream.Recv()
			if errors.Is(err, io.EOF) {
				return
			}

			if err != nil {
				log.Printf("Stream error: %v\n", err)
				return
			}

			content := response.Choices[0].Delta.Content
			ch <- content
		}
	}()

	return ch, nil
}
