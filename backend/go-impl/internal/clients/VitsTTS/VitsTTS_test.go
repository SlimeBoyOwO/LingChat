package VitsTTS

import (
	"context"
	"io"
	"testing"
	"time"
)

func TestVoiceVITS(t *testing.T) {
	// 初始化客户端
	client := NewClient("https://artrajz-vits-simple-api.hf.space", "")
	client.SpeakerID = 164 // 设置测试用的 speaker ID

	// 测试文本
	text := "你好,こんにちは"

	// 测试 VoiceVITS 方法
	ctx := context.Background()
	audioData, err := client.VoiceVITS(ctx, text)
	if err != nil {
		t.Fatalf("VoiceVITS failed: %v", err)
	}

	// 检查返回的音频数据
	if len(audioData) == 0 {
		t.Error("Expected non-empty audio data")
	}
}

func TestVoiceVITSStream(t *testing.T) {
	// 初始化客户端
	client := NewClient("https://artrajz-vits-simple-api.hf.space", "")
	client.SpeakerID = 164 // 设置测试用的 speaker ID

	// 测试文本
	text := "你好,こんにちは"

	// 测试 VoiceVITSStream 方法
	ctx := context.Background()
	stream, err := client.VoiceVITSStream(ctx, text)
	if err != nil {
		t.Fatalf("VoiceVITSStream failed: %v", err)
	}

	// 检查流是否有效
	if stream == nil {
		t.Error("Expected non-nil stream")
	}

	// 创建一个带超时的上下文
	ctx, cancel := context.WithTimeout(context.Background(), 200*time.Second)
	defer cancel()

	// 使用带超时的上下文读取数据
	done := make(chan struct{})
	var b []byte
	var readErr error

	go func() {
		b, readErr = io.ReadAll(stream)
		close(done)
	}()

	select {
	case <-ctx.Done():
		t.Fatal("Timeout while reading stream data")
	case <-done:
		if readErr != nil {
			t.Fatalf("ReadAll failed: %v", readErr)
		}
		if len(b) == 0 {
			t.Error("Expected non-empty stream data")
		}
	}
}

func TestVoiceVITS_Concurrent(t *testing.T) {
	// 初始化客户端
	client := NewClient("https://artrajz-vits-simple-api.hf.space", "")
	client.SpeakerID = 164 // 设置测试用的 speaker ID

	// 测试文本
	text := "你好,こんにちは"

	// 测试 VoiceVITS 方法
	ctx := context.Background()
	for range 3 {
		go func() {
			audioData, err := client.VoiceVITS(ctx, text)
			if err != nil {
				t.Fatalf("VoiceVITS failed: %v", err)
			}

			// 检查返回的音频数据
			if len(audioData) == 0 {
				t.Error("Expected non-empty audio data")
			}
		}()
	}

	time.Sleep(20 * time.Second)
}
