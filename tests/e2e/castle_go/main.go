package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"os/signal"
	"strconv"
	"syscall"

	"github.com/openosaka/castled/sdk/go/castle"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

var rootCmd = &cobra.Command{
	Use:    "castle",
	Hidden: true,
}

var httpCmd = &cobra.Command{
	Use:  "http",
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		localPort, err := strconv.Atoi(args[0])
		if err != nil {
			return err
		}

		serverAddr, err := cmd.Flags().GetString("server-addr")
		if err != nil {
			return err
		}

		var options []castle.HTTPOption
		if domain, _ := cmd.Flags().GetString("domain"); domain != "" {
			println(domain)
			options = append(options, castle.WithHTTPDomain(domain))
		} else if subdomain, _ := cmd.Flags().GetString("subdomain"); subdomain != "" {
			options = append(options, castle.WithHTTPSubDomain(subdomain))
		} else if randomSubdomain, _ := cmd.Flags().GetBool("random-subdomain"); randomSubdomain {
			options = append(options, castle.WithHTTPRandomSubdomain())
		} else if remotePort, _ := cmd.Flags().GetUint16("remote-port"); remotePort != 0 {
			options = append(options, castle.WithHTTPPort(remotePort))
		}

		tunnel := castle.NewHTTPTunnel("go-http", getLocalAddr(cmd.Flags(), localPort), options...)
		return run(cmd.Context(), serverAddr, tunnel)
	},
}

var tcpCmd = &cobra.Command{
	Use:  "tcp",
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		localPort, err := strconv.Atoi(args[0])
		if err != nil {
			return err
		}

		serverAddr, err := cmd.Flags().GetString("server-addr")
		if err != nil {
			return err
		}

		var options []castle.TCPOption
		if remotePort, _ := cmd.Flags().GetUint16("remote-port"); remotePort != 0 {
			options = append(options, castle.WithTCPPort(remotePort))
		}

		tunnel := castle.NewTCPTunnel("go-tcp", getLocalAddr(cmd.Flags(), localPort), options...)
		return run(cmd.Context(), serverAddr, tunnel)
	},
}

var udpCmd = &cobra.Command{
	Use:  "udp",
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		localPort, err := strconv.Atoi(args[0])
		if err != nil {
			return err
		}
		serverAddr, err := cmd.Flags().GetString("server-addr")
		if err != nil {
			return err
		}

		var options []castle.UDPOption
		if remotePort, _ := cmd.Flags().GetUint16("remote-port"); remotePort != 0 {
			options = append(options, castle.WithUdpPort(remotePort))
		}

		tunnel := castle.NewUDPTunnel("go-udp", getLocalAddr(cmd.Flags(), localPort), options...)
		return run(cmd.Context(), serverAddr, tunnel)
	},
}

func getLocalAddr(fs *pflag.FlagSet, port int) string {
	localHost, _ := fs.GetString("local-host")
	return fmt.Sprintf("%s:%d", localHost, port)
}

func run(ctx context.Context, serverAddr string, tunnel *castle.Tunnel) error {
	client, err := castle.NewClient(serverAddr)
	if err != nil {
		return err
	}
	entrypoint, quit, err := client.StartTunnel(ctx, tunnel)
	if err != nil {
		return err
	}
	log.Printf("Entrypoint: %v", entrypoint)
	return <-quit
}

func init() {
	rootCmd.PersistentFlags().String("server-addr", "127.0.0.1:6610", "")

	httpCmd.Flags().String("domain", "", "Domain")
	httpCmd.Flags().String("subdomain", "", "")
	httpCmd.Flags().Bool("random-subdomain", false, "Random subdomain")
	httpCmd.Flags().Uint16("remote-port", 0, "Remote port")
	httpCmd.Flags().String("local-host", "127.0.0.1", "Domain")

	tcpCmd.Flags().Uint16("remote-port", 0, "Remote port")
	tcpCmd.Flags().String("local-host", "127.0.0.1", "Domain")

	udpCmd.Flags().Uint16("remote-port", 0, "Remote port")
	udpCmd.Flags().String("local-host", "127.0.0.1", "Domain")

	rootCmd.AddCommand(tcpCmd, udpCmd, httpCmd)
}

func main() {
	ctx := context.Background()
	ctx, cancel := signal.NotifyContext(ctx, os.Interrupt, syscall.SIGTERM)
	defer cancel()

	if _, err := rootCmd.ExecuteContextC(ctx); err != nil {
		log.Fatal(err)
	}
}
