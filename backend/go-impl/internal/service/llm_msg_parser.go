package service

import (
	"fmt"
	"path/filepath"
	"regexp"
	"strings"
)

// Result 表示parse结果
type Result struct {
	Index         int     `json:"index"`
	OriginalTag   string  `json:"original_tag"`
	FollowingText string  `json:"following_text"`
	MotionText    string  `json:"motion_text"`
	JapaneseText  string  `json:"japanese_text"`
	Predicted     string  `json:"predicted"`
	Confidence    float64 `json:"confidence"`
	VoiceFile     string  `json:"voice_file"`
}

// AnalyzeEmotions 分析文本中每个【】标记的情绪，并提取日语和中文部分
func AnalyzeEmotions(text string, tempVoiceDir string, ttsFormat string) []Result {
	// 正则表达式查找情绪段落
	emotionRegex := regexp.MustCompile(`(【(.*?)】)([^【】]*)`)
	matches := emotionRegex.FindAllStringSubmatch(text, -1)

	var results []Result

	for i, match := range matches {
		emotionTag := match[2]    // 【】内的内容
		followingText := match[3] // 标签后面的文本

		// 统一处理括号（兼容中英文括号）
		followingText = strings.ReplaceAll(followingText, "(", "（")
		followingText = strings.ReplaceAll(followingText, ")", "）")

		// 提取日语部分（<...>）
		japaneseRegex := regexp.MustCompile(`<(.*?)>`)
		japaneseMatch := japaneseRegex.FindStringSubmatch(followingText)
		japaneseText := ""
		if len(japaneseMatch) > 1 {
			japaneseText = strings.TrimSpace(japaneseMatch[1])
		}

		// 提取动作部分（（...））
		motionRegex := regexp.MustCompile(`（(.*?)）`)
		motionMatch := motionRegex.FindStringSubmatch(followingText)
		motionText := ""
		if len(motionMatch) > 1 {
			motionText = strings.TrimSpace(motionMatch[1])
		}

		// 清理后的文本（移除日语部分和动作部分）
		cleanRegex := regexp.MustCompile(`<.*?>|（.*?）`)
		cleanedText := strings.TrimSpace(cleanRegex.ReplaceAllString(followingText, ""))

		// 清理日语文本中的动作部分
		if japaneseText != "" {
			japaneseCleanRegex := regexp.MustCompile(`（.*?）`)
			japaneseText = strings.TrimSpace(japaneseCleanRegex.ReplaceAllString(japaneseText, ""))
		}

		// 跳过完全空的文本
		if followingText == "" && japaneseText == "" && motionText == "" {
			continue
		}

		// TODO: 省略了原语言检测和交换逻辑

		voiceFile := filepath.Join(tempVoiceDir, fmt.Sprintf("part_%d.%s", i+1, ttsFormat))

		results = append(results, Result{
			Index:         i + 1,
			OriginalTag:   emotionTag,
			FollowingText: cleanedText,
			MotionText:    motionText,
			JapaneseText:  japaneseText,
			VoiceFile:     voiceFile,
		})
	}

	return results
}
