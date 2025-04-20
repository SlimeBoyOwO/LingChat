package VitsTTS

import (
	"context"
	"fmt"
	"io"
	"strconv"
	"time"

	"LingChat/internal/config"

	"github.com/go-resty/resty/v2"
)

type Client struct {
	resty.Client
	URL     string
	TempDir string

	SpeakerID   int
	AudioFormat string
	Lang        string
	Enable      bool
}

func NewClient(url string, tempDir string) *Client {
	httpClient := resty.New()
	httpClient.SetTimeout(time.Second * 120)
	conf := config.GetConfigFromEnv()

	return &Client{
		Client:      *httpClient,
		URL:         url,
		TempDir:     tempDir,
		SpeakerID:   conf.Vits.SpeakerID,
		AudioFormat: "wav",
		Enable:      true,
	}
}

func (c *Client) VoiceVITS(ctx context.Context, text string) ([]byte, error) {
	resp, err := c.R().
		SetContext(ctx).
		SetQueryParams(map[string]string{
			"text": text,
			"id":   strconv.Itoa(c.SpeakerID),
		}).
		Get(c.URL)
	if err != nil {
		return nil, err
	}
	if !resp.IsSuccess() {
		return nil, fmt.Errorf("VITS TTS request failed with status code: %d", resp.StatusCode())
	}

	return resp.Body(), nil
}

func (c *Client) VoiceVITSStream(ctx context.Context, text string) (io.ReadCloser, error) {
	resp, err := c.R().
		SetContext(ctx).
		SetQueryParams(map[string]string{
			"text":      text,
			"id":        strconv.Itoa(c.SpeakerID),
			"streaming": "true",
		}).
		SetDoNotParseResponse(true).
		Get(c.URL)
	if err != nil {
		return nil, err
	}
	if !resp.IsSuccess() {
		resp.RawResponse.Body.Close()
		return nil, fmt.Errorf("VITS TTS streaming request failed with status code: %d", resp.StatusCode())
	}

	// TODO: 加缓冲？以下实现是不对的不能直接用
	// // 创建缓冲区
	// buf := new(bytes.Buffer)
	//
	// // 启动后台goroutine处理流数据
	// go func() {
	// 	rawBody := resp.RawBody()
	// 	defer rawBody.Close() // 确保连接最终被关闭
	//
	// 	// 将响应体复制到缓冲区
	// 	_, err := io.Copy(buf, rawBody)
	// 	if err != nil && !errors.Is(err, io.EOF) {
	// 		// 可选：记录错误
	// 		log.Printf("Error reading response body: %v", err)
	// 	}
	// }()

	return resp.RawBody(), nil
}
