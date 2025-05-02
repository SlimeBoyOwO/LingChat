package v1

import (
	"net/http"

	"github.com/gin-gonic/gin"
	"github.com/sashabaranov/go-openai"

	"LingChat/internal/service"
)

type ChatRoute struct {
	lingChatService *service.LingChatService
}

func NewChatRoute(lingChatService *service.LingChatService) *ChatRoute {
	return &ChatRoute{
		lingChatService: lingChatService,
	}
}

func (c *ChatRoute) RegisterRoute(r *gin.RouterGroup) {
	rg := r.Group("/v1/chat")
	{
		rg.GET("/history", c.getChatHistory)
		rg.POST("/history", c.loadChatHistory)
	}
}

func (c *ChatRoute) getChatHistory(ctx *gin.Context) {
	history := c.lingChatService.GetChatHistory(ctx)
	ctx.JSON(http.StatusOK, history)
}

func (c *ChatRoute) loadChatHistory(ctx *gin.Context) {
	var messages []openai.ChatCompletionMessage
	if err := ctx.ShouldBindJSON(&messages); err != nil {
		ctx.JSON(http.StatusBadRequest, gin.H{
			"error": "无效的聊天历史记录格式: " + err.Error(),
		})
		return
	}

	response := c.lingChatService.LoadChatHistory(ctx, messages)

	ctx.JSON(http.StatusOK, response)
}
