package jwt

import (
	"fmt"
	"strings"
	"time"

	"github.com/golang-jwt/jwt/v5"
)

type ClaimParams struct {
	UserID  int    `json:"user_id"`
	TokenID string `json:"token_id"`
}

type CustomClaims struct {
	ClaimParams
	jwt.RegisteredClaims
}

type JWT struct {
	secret []byte
	issuer string
}

func NewJWT(secret []byte, issuer string) *JWT {
	return &JWT{
		secret: secret,
		issuer: issuer,
	}
}

func (j *JWT) GenerateToken(params ClaimParams, duration time.Duration) (string, error) {
	if duration == 0 {
		duration = 7 * 24 * time.Hour
	}
	issuedAt := time.Now()
	expiresAt := issuedAt.Add(duration)

	claims := CustomClaims{
		params,
		jwt.RegisteredClaims{
			Issuer: j.issuer,

			ExpiresAt: jwt.NewNumericDate(expiresAt),
			NotBefore: jwt.NewNumericDate(issuedAt),
			IssuedAt:  jwt.NewNumericDate(issuedAt),
		},
	}
	withClaims := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	signedString, err := withClaims.SignedString(j.secret)
	if err != nil {
		return "", err
	}

	return fmt.Sprint("Bearer ", signedString), nil
}

func (j *JWT) ParseToken(input string) (*CustomClaims, error) {
	if !strings.HasPrefix(input, "Bearer ") {
		return nil, fmt.Errorf("invalid token format")
	}
	tokenString := strings.TrimPrefix(input, "Bearer ")

	// 解析Token
	token, err := jwt.ParseWithClaims(tokenString, &CustomClaims{},
		func(token *jwt.Token) (interface{}, error) {
			// 验证算法是否匹配
			if _, ok := token.Method.(*jwt.SigningMethodHMAC); !ok {
				return nil, fmt.Errorf("未预期的签名方法: %v", token.Header["alg"])
			}
			return j.secret, nil
		},
	)
	if err != nil {
		return nil, err
	}

	claims, ok := token.Claims.(*CustomClaims)
	if !(ok && token.Valid) {
		return nil, fmt.Errorf("invalid token")
	}

	return claims, nil
}
