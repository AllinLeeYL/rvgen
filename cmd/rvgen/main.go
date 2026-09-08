package main

import (
	"errors"
	"flag"
	"fmt"
	"os"
	"rvgen"
)

func main() {
	if err := rvgen.Run(os.Args[1:], os.Stdout, os.Stderr); err != nil && !errors.Is(err, flag.ErrHelp) {
		fmt.Fprintln(os.Stderr, "rvgen:", err)
		os.Exit(1)
	}
}
