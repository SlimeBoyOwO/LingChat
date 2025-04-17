package emotionPredictor

import (
	"context"
	"fmt"
	"time"

	"github.com/go-resty/resty/v2"
)

type Client struct {
	resty.Client
	URL string
}

func NewClient(url string) *Client {
	httpClient := resty.New()
	httpClient.SetTimeout(time.Second * 120)
	return &Client{
		Client: *httpClient,
		URL:    url,
	}
}

func (c *Client) Predict(ctx context.Context, text string, confidenceThreshold float64) (*PredictionResponse, error) {
	result := &PredictionResponse{}
	resp, err := c.R().
		SetContext(ctx).
		SetHeader("Content-Type", "application/json").
		SetBody(map[string]interface{}{
			"text":                 text,
			"confidence_threshold": confidenceThreshold,
		}).
		SetResult(&result).
		Post(c.URL + "/predict")
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}

	if !resp.IsSuccess() {
		return nil, fmt.Errorf("API returned error status: %d, body: %s", resp.StatusCode(), resp.Body())
	}

	return result, nil

}
