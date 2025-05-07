package v1

import (
	"net/http"

	"github.com/gin-gonic/gin"
	"github.com/sashabaranov/go-openai"

	"LingChat/api/routes/middleware"
	"LingChat/internal/data"
	"LingChat/internal/service"
	"LingChat/pkg/jwt"
)

type ChatRoute struct {
	lingChatService *service.LingChatService
	userRepo        data.UserRepo
	jwt             *jwt.JWT
}

func NewChatRoute(lingChatService *service.LingChatService, userRepo data.UserRepo, jwt *jwt.JWT) *ChatRoute {
	return &ChatRoute{
		lingChatService: lingChatService,
		userRepo:        userRepo,
		jwt:             jwt,
	}
}

func (c *ChatRoute) RegisterRoute(r *gin.RouterGroup) {
	rg := r.Group("/v1/chat")
	{
		rg.GET("/history", middleware.TokenAuth(false, c.jwt, c.userRepo), c.getChatHistory)
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
