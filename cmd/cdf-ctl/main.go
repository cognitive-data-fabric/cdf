package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "cdf-ctl",
	Short: "CDF Admin CLI",
	Long:  "Command-line tool for managing the Cognitive Data Fabric cluster",
}

var gatewayAddr string

func init() {
	rootCmd.PersistentFlags().StringVarP(&gatewayAddr, "gateway", "g", "http://localhost:8080", "CDF gateway address")
}

var queryCmd = &cobra.Command{
	Use:   "query [cql]",
	Short: "Execute a CQL query",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		cql := args[0]
		reqBody, _ := json.Marshal(map[string]string{
			"request_id": fmt.Sprintf("cli-%d", time.Now().Unix()),
			"cql":        cql,
		})

		resp, err := http.Post(gatewayAddr+"/v1/query", "application/json", bytes.NewReader(reqBody))
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		defer resp.Body.Close()

		body, _ := io.ReadAll(resp.Body)
		fmt.Println(string(body))
	},
}

var insertCmd = &cobra.Command{
	Use:   "insert [table] [json]",
	Short: "Insert a document",
	Args:  cobra.ExactArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		table := args[0]
		data := args[1]

		var jsonData map[string]interface{}
		if err := json.Unmarshal([]byte(data), &jsonData); err != nil {
			fmt.Fprintf(os.Stderr, "Invalid JSON: %v\n", err)
			os.Exit(1)
		}

		reqBody, _ := json.Marshal(map[string]interface{}{
			"request_id": fmt.Sprintf("cli-%d", time.Now().Unix()),
			"table":      table,
			"data":       jsonData,
		})

		resp, err := http.Post(gatewayAddr+"/v1/insert", "application/json", bytes.NewReader(reqBody))
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		defer resp.Body.Close()

		body, _ := io.ReadAll(resp.Body)
		fmt.Println(string(body))
	},
}

var searchCmd = &cobra.Command{
	Use:   "search [table]",
	Short: "Vector similarity search",
	Run: func(cmd *cobra.Command, args []string) {
		if len(args) < 1 {
			fmt.Fprintln(os.Stderr, "Usage: cdf-ctl search <table> --vector=<...>")
			os.Exit(1)
		}

		table := args[0]
		vectorStr, _ := cmd.Flags().GetString("vector")
		topK, _ := cmd.Flags().GetInt("top-k")

		// Parse vector from string
		vectorStr = strings.Trim(vectorStr, "[]")
		parts := strings.Split(vectorStr, ",")
		var vector []float32
		for _, p := range parts {
			var v float32
			fmt.Sscanf(strings.TrimSpace(p), "%f", &v)
			vector = append(vector, v)
		}

		reqBody, _ := json.Marshal(map[string]interface{}{
			"request_id": fmt.Sprintf("cli-%d", time.Now().Unix()),
			"table":      table,
			"vector":     vector,
			"top_k":      topK,
		})

		resp, err := http.Post(gatewayAddr+"/v1/search", "application/json", bytes.NewReader(reqBody))
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		defer resp.Body.Close()

		body, _ := io.ReadAll(resp.Body)
		fmt.Println(string(body))
	},
}

var healthCmd = &cobra.Command{
	Use:   "health",
	Short: "Check cluster health",
	Run: func(cmd *cobra.Command, args []string) {
		resp, err := http.Get(gatewayAddr + "/health")
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		defer resp.Body.Close()

		body, _ := io.ReadAll(resp.Body)
		fmt.Println(string(body))
	},
}

var clusterCmd = &cobra.Command{
	Use:   "cluster",
	Short: "Cluster management",
}

var clusterStatusCmd = &cobra.Command{
	Use:   "status",
	Short: "Show cluster status",
	Run: func(cmd *cobra.Command, args []string) {
		// Query the gateway for live cluster status. Falls back to a clear
		// "unknown" message if the gateway is unreachable so operators don't
		// get a misleading "all healthy" reading from hardcoded values.
		resp, err := http.Get(gatewayAddr + "/v1/admin/cluster/status")
		if err != nil {
			fmt.Fprintf(os.Stderr, "Warning: cannot reach gateway at %s: %v\n", gatewayAddr, err)
			fmt.Println("Cluster Status: <unreachable>")
			return
		}
		defer resp.Body.Close()

		if resp.StatusCode != http.StatusOK {
			fmt.Fprintf(os.Stderr, "Gateway returned status %d\n", resp.StatusCode)
			fmt.Println("Cluster Status: <error>")
			return
		}

		body, _ := io.ReadAll(resp.Body)
		fmt.Println("Cluster Status:")
		fmt.Println(string(body))
	},
}

func main() {
	searchCmd.Flags().String("vector", "", "Query vector [v1,v2,...]")
	searchCmd.Flags().Int("top-k", 10, "Number of results")

	rootCmd.AddCommand(queryCmd, insertCmd, searchCmd, healthCmd)
	clusterCmd.AddCommand(clusterStatusCmd)
	rootCmd.AddCommand(clusterCmd)

	if err := rootCmd.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
