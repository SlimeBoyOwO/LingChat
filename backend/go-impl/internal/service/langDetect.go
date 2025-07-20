package service

import (
	"github.com/pemistahl/lingua-go"
)

type LanguageDetector struct {
	lingua.LanguageDetector
}

func NewLanguageDetector() LanguageDetector {
	detect := lingua.NewLanguageDetectorBuilder().
		FromAllSpokenLanguages().
		Build()
	return LanguageDetector{detect}
}

func (l *LanguageDetector) detectLanguage(text string) lingua.Language {
	lang, ok := l.DetectLanguageOf(text)
	if !ok {
		return lingua.Unknown
	}
	// confidence := l.ComputeLanguageConfidence(text, lang)
	// if confidence > 0.5 {
	// 	return lang
	// }
	return lang
}

func (l *LanguageDetector) DetectLanguage(text string) string {
	lang := l.detectLanguage(text)
	// 只返回中文、日文或未知
	switch lang {
	case lingua.Japanese:
		return "Japanese"
	case lingua.Chinese:
		return "Chinese"
	default:
		return "Unknown"
	}
}
