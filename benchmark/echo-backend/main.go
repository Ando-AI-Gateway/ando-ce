// Minimal HTTP echo backend for benchmarking.
// Responds instantly with 200 OK to any request.
// Designed to NOT be the bottleneck — pure Go net/http.
package main

import (
	"flag"
	"fmt"
	"net/http"
	"os"
)

func main() {
	port := flag.String("port", "3000", "Listen port")
	flag.Parse()

	mux := http.NewServeMux()

	// Simple 200 OK for any path
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		w.WriteHeader(http.StatusOK)
		fmt.Fprint(w, "ok")
	})

	addr := ":" + *port
	fmt.Fprintf(os.Stderr, "[echo-backend] listening on http://0.0.0.0%s\n", addr)
	if err := http.ListenAndServe(addr, mux); err != nil {
		fmt.Fprintf(os.Stderr, "fatal: %v\n", err)
		os.Exit(1)
	}
}
