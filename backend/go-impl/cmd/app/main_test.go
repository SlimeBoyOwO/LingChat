package main

import (
	"encoding/json"
	"fmt"
	"log"
	"sync"
	"testing"
	"time"

	"github.com/gorilla/websocket"

	"LingChat/api"
)

func Test_ws(t *testing.T) {
	serverAddr := "ws://localhost:8765/ws" // Replace with your server address
	numClients := 10

	var wg sync.WaitGroup
	wg.Add(numClients)

	for i := 0; i < numClients; i++ {
		clientID := i
		go func() {
			defer wg.Done()
			runClient(serverAddr, clientID)
		}()
		// Small delay to avoid overwhelming connection attempts
		time.Sleep(time.Millisecond * 50)
	}

	wg.Wait()
	fmt.Println("Test completed")
}

func runClient(serverAddr string, clientID int) {
	// Connect to WebSocket server
	conn, _, err := websocket.DefaultDialer.Dial(serverAddr, nil)
	if err != nil {
		log.Printf("Client %d: Connection error: %v", clientID, err)
		return
	}
	defer conn.Close()

	log.Printf("Client %d: Connected successfully", clientID)

	// Create unique message for this client
	msg := api.Message{
		Type:    "test",
		Content: fmt.Sprintf("Message from client %d", clientID),
	}

	// Serialize to JSON
	jsonData, err := json.Marshal(msg)
	if err != nil {
		log.Printf("Client %d: JSON encoding error: %v", clientID, err)
		return
	}

	// Send the message
	for _ = range 100 {
		if err := conn.WriteMessage(websocket.TextMessage, jsonData); err != nil {
			log.Printf("Client %d: Send error: %v", clientID, err)
			return
		}
	}

	log.Printf("Client %d: Sent: %s", clientID, string(jsonData))

	// Optionally wait for response
	_, response, err := conn.ReadMessage()
	if err != nil {
		log.Printf("Client %d: Read error: %v", clientID, err)
		return
	}

	log.Printf("Client %d: Received: %s", clientID, string(response))
}
