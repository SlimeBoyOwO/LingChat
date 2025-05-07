package common

import (
	"github.com/gin-gonic/gin"

	"LingChat/internal/data/ent/ent"
)

const (
	CurrentUserInfoKey = "current-user-info"
)

func GetCurrentUserInfo(c *gin.Context) *ent.User {
	val, exist := c.Get(CurrentUserInfoKey)
	if !exist {
		return nil
	}
	return val.(*ent.User)
}
