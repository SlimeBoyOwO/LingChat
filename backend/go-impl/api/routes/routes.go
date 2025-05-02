package routes

import (
	"errors"
	"log"
	"net/http"
	"reflect"

	"github.com/gin-gonic/gin"

	"LingChat/api/routes/v1"
)

func NewEngine() (*gin.Engine, error) {
	r := gin.New()
	r.Use(
		gin.Logger(),
		gin.Recovery(),
	)

	return r, nil
}

type HttpEngine struct {
	Engine *gin.Engine
	Addr   string

	ChatRouteV1 *v1.ChatRoute
}

func NewHTTPEngine(addr string, chatRoute *v1.ChatRoute) *HttpEngine {

	engine, err := NewEngine()
	if err != nil {
		log.Fatal(err)
	}

	return &HttpEngine{
		Engine:      engine,
		Addr:        addr,
		ChatRouteV1: chatRoute,
	}
}

type Route interface {
	RegisterRoute(r *gin.RouterGroup)
}

func (h *HttpEngine) RegisterRoutes() {
	eg := h.Engine.Group("/api")

	v := reflect.ValueOf(*h)
	for i := 0; i < v.NumField(); i++ {
		if router, ok := v.Field(i).Interface().(Route); ok {
			router.RegisterRoute(eg)
		}
	}
}

func (h *HttpEngine) Run() (*http.Server, error) {
	h.RegisterRoutes()
	srv := &http.Server{
		Addr:    h.Addr,
		Handler: h.Engine,
	}

	go func() {
		if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatalf("listen: %s\n", err)
		}
	}()
	return srv, nil
}
