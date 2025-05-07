package middleware

import (
	"context"
	"net/http"

	"github.com/gin-gonic/gin"

	"LingChat/api/routes/common"
	"LingChat/internal/data"
	"LingChat/pkg/jwt"
)

func TokenAuth(mustLogin bool, jwt *jwt.JWT, userRepo data.UserRepo) gin.HandlerFunc {
	return func(c *gin.Context) {
		token := c.GetHeader("Authorization")
		if token == "" {
			token, _ = c.Cookie("token")
		}

		if token == "" {
			if mustLogin {
				c.JSON(http.StatusUnauthorized, gin.H{
					"code": http.StatusUnauthorized,
					"msg":  "no token",
				})
				c.Abort()
			}
			return
		}

		claims, err := jwt.ParseToken(token)
		if err != nil {
			if mustLogin {
				c.JSON(http.StatusUnauthorized, gin.H{
					"code": http.StatusUnauthorized,
					"msg":  "invalid token",
				})
				c.Abort()
			}
			return
		}

		user, err := userRepo.GetByID(c.Request.Context(), int64(claims.UserID))
		if err != nil {
			if mustLogin {
				c.JSON(http.StatusUnauthorized, gin.H{
					"code": http.StatusUnauthorized,
					"msg":  "no valid user",
				})
				c.Abort()
			}
			return
		}

		c.Set(common.CurrentUserInfoKey, user)
		if c.Request != nil {
			ctx := context.WithValue(c.Request.Context(), common.CurrentUserInfoKey, user)
			c.Request = c.Request.WithContext(ctx)
		}

	}
}
