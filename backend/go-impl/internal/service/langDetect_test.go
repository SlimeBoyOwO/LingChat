package service

import (
	"testing"
)

func TestDetectLanguage(t *testing.T) {

	ld := NewLanguageDetector()

	tests := []struct {
		name     string
		input    string
		expected string
	}{
		// 纯中文测试
		{
			name:     "纯中文",
			input:    "你好，世界！",
			expected: "Chinese",
		},
		{
			name:     "中文长句",
			input:    "今天天气真好，我们去公园散步吧。",
			expected: "Chinese",
		},
		{
			name:     "中文带标点",
			input:    "《论语》中说：\"学而时习之，不亦说乎？\"",
			expected: "Chinese",
		},
		{
			name:     "中文繁体",
			input:    "你好，世界！今天天氣真好，我們去公園散步吧。",
			expected: "Chinese",
		},

		// 纯日文测试
		{
			name:     "纯日文平假名",
			input:    "こんにちは、世界！",
			expected: "Japanese",
		},
		{
			name:     "纯日文片假名",
			input:    "コンピュータ",
			expected: "Japanese",
		},
		{
			name:     "日文长句",
			input:    "今日はとても良い天気ですね。公園へ散歩に行きましょう。",
			expected: "Japanese",
		},
		{
			name:     "纯日文汉字",
			input:    "今日天気公園散歩",
			expected: "Japanese",
		},
		{
			name:     "日文汉字长句",
			input:    "今日の天気はとても良いです。公園で散歩をしましょう。",
			expected: "Japanese",
		},

		// 中日混合测试
		{
			name:     "中日混合1",
			input:    "你好こんにちは",
			expected: "Unknown",
		},
		{
			name:     "中日混合2",
			input:    "今日はとても良い天気ですね。我们去公园散步吧。",
			expected: "Unknown",
		},
		{
			name:     "中日混合3",
			input:    "コンピュータ电脑",
			expected: "Unknown",
		},
		{
			name:     "中日混合4（繁体）",
			input:    "今日はとても良い天気ですね。我們去公園散步吧。",
			expected: "Unknown",
		},

		// 其他语言测试
		{
			name:     "英文",
			input:    "what language is this!",
			expected: "Unknown",
		},
		{
			name:     "韩文",
			input:    "안녕하세요",
			expected: "Unknown",
		},
		{
			name:     "法文",
			input:    "Bonjour le monde",
			expected: "Unknown",
		},

		// 边界情况测试
		{
			name:     "空字符串",
			input:    "",
			expected: "Unknown",
		},
		{
			name:     "纯标点",
			input:    "！？。、",
			expected: "Unknown",
		},
		{
			name:     "数字",
			input:    "123456",
			expected: "Unknown",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ld.DetectLanguage(tt.input)
			if result != tt.expected {
				t.Errorf("DetectLanguage(%q) = %v, want %v", tt.input, result, tt.expected)
			}
		})
	}
}
