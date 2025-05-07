package routes

import (
	"errors"
	"log"
	"net/http"

	"github.com/gin-gonic/gin"
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

	// ChatRouteV1 *v1.ChatRoute
	routes []Route
}

func NewHTTPEngine(addr string, routes ...Route) *HttpEngine {

	engine, err := NewEngine()
	if err != nil {
		log.Fatal(err)
	}

	return &HttpEngine{
		Engine: engine,
		Addr:   addr,
		// ChatRouteV1: chatRoute,
		routes: routes,
	}
}

type Route interface {
	RegisterRoute(r *gin.RouterGroup)
}

func (h *HttpEngine) RegisterRoutes() {
	eg := h.Engine.Group("/api")

	// v := reflect.ValueOf(*h)
	// for i := 0; i < v.NumField(); i++ {
	// 	if router, ok := v.Field(i).Interface().(Route); ok {
	// 		router.RegisterRoute(eg)
	// 	}
	// }
	for _, route := range h.routes {
		route.RegisterRoute(eg)
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
