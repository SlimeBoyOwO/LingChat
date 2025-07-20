package emotionPredictor

import (
	"context"
	"fmt"
	"testing"
)

func TestPredict(t *testing.T) {
	client := NewClient("http://localhost:8000")

	ctx := context.Background()
	resp, err := client.Predict(ctx, "今天天气真好", 0.08)
	if err != nil {
		t.Fatalf("Predict failed: %v", err)
	}
	fmt.Println(resp)
}
