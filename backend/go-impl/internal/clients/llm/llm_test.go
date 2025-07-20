package llm

import (
	"context"
	"fmt"
	"testing"
	"time"
)

func TestLLMClient_Chat(t *testing.T) {
	client := NewLLMClient("", "")
	ctx := context.Background()

	// 测试基本聊天功能
	response, err := client.Chat(ctx, "你好", "deepseek-chat")
	if err != nil {
		t.Fatalf("Chat failed: %v", err)
	}

	if response == "" {
		t.Error("Expected non-empty response")
	}

	fmt.Println("Response:", response)
}

func TestLLMClient_ChatStream(t *testing.T) {
	client := NewLLMClient("", "")
	ctx := context.Background()

	// 测试流式聊天功能
	ch, err := client.ChatStream(ctx, "你好")
	if err != nil {
		t.Fatalf("ChatStream failed: %v", err)
	}

	// 收集流式响应
	var fullResponse string
	timeout := time.After(10 * time.Second)
	done := make(chan bool)

	go func() {
		for content := range ch {
			fullResponse += content
		}
		done <- true
	}()

	// 等待响应完成或超时
	select {
	case <-done:
		// 正常完成
	case <-timeout:
		t.Fatal("Stream response timeout")
	}

	if fullResponse == "" {
		t.Error("Expected non-empty stream response")
	}
}

func TestLLMClient_ConcurrentAccess(t *testing.T) {
	client := NewLLMClient("", "")
	ctx := context.Background()

	// 并发测试
	concurrency := 5
	done := make(chan bool)

	for i := 0; i < concurrency; i++ {
		go func() {
			_, err := client.Chat(ctx, "你好", "deepseek-chat")
			if err != nil {
				t.Errorf("Concurrent chat failed: %v", err)
			}
			done <- true
		}()
	}

	// 等待所有goroutine完成
	for i := 0; i < concurrency; i++ {
		<-done
	}
}
