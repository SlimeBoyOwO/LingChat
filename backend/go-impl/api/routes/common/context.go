package common

import (
	"context"

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

func GetUserFromContext(ctx context.Context) *ent.User {
	val := ctx.Value(CurrentUserInfoKey)
	if val == nil {
		// context 中没有这个 key 对应的值
		return nil
	}

	user, ok := val.(*ent.User)
	if !ok {
		// 值存在，但不是 *ent.User 类型
		return nil
	}

	return user
}
