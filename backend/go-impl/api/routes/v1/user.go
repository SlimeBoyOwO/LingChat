package v1

import (
	"net/http"
	"time"

	"github.com/gin-gonic/gin"

	"LingChat/internal/data"
	"LingChat/internal/data/ent/ent"
	"LingChat/internal/service"
)

type UserRoute struct {
	userService service.UserService
}

func NewUserRoute(userService service.UserService) *UserRoute {
	return &UserRoute{userService: userService}
}

func (u *UserRoute) RegisterRoute(r *gin.RouterGroup) {
	rg := r.Group("/v1/user")
	{
		rg.POST("/register", u.register)
		rg.POST("/login", u.login)
		rg.POST("/password")
	}
}

// register 用户注册处理
func (u *UserRoute) register(c *gin.Context) {
	var req UserRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{
			"code": http.StatusBadRequest,
			"msg":  "Invalid request: " + err.Error(),
		})
		return
	}

	user, token, err := u.userService.Register(c.Request.Context(), &data.User{
		Username: req.Username,
		Password: req.Password,
		Email:    req.Email,
	})
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{
			"code": http.StatusInternalServerError,
			"msg":  err.Error(),
		})
		return
	}

	// 设置Cookie
	c.SetCookie("token", token, 86400, "/", "", false, true) // 1天有效期

	c.JSON(http.StatusOK, gin.H{
		"code": http.StatusOK,
		"msg":  "Registration successful",
		"data": gin.H{
			"user_id":  user.ID,
			"username": user.Username,
			"email":    user.Email,
			"token":    token,
		},
	})
}

// login 用户登录处理
func (u *UserRoute) login(c *gin.Context) {
	var req UserRequest
	var user *ent.User
	var token string
	var err error
	if c.Request.ContentLength != 0 {
		if err := c.ShouldBindJSON(&req); err != nil {
			c.JSON(http.StatusBadRequest, gin.H{
				"code": http.StatusBadRequest,
				"msg":  "Invalid request: " + err.Error(),
			})
			return
		}

		user, token, err = u.userService.Login(c.Request.Context(), &data.User{
			Username: req.Username,
			Password: req.Password,
		})
	} else {
		user, token, err = u.userService.Login(c.Request.Context(), nil)
	}
	if err != nil {
		c.JSON(http.StatusUnauthorized, gin.H{
			"code": http.StatusUnauthorized,
			"msg":  err.Error(),
		})
		return
	}

	// 设置Cookie
	c.SetCookie("token", token, int(7*24*time.Hour), "/", "", false, true) // 7天有效期

	c.JSON(http.StatusOK, gin.H{
		"code": http.StatusOK,
		"msg":  "Login successful",
		"data": gin.H{
			"user_id":  user.ID,
			"username": user.Username,
			"email":    user.Email,
			"token":    token,
		},
	})
}

// UserRequest 用户请求结构
type UserRequest struct {
	Username string `json:"username"`
	Password string `json:"password"`
	Email    string `json:"email"`
}
