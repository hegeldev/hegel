package libhegel

import (
	"io"
	"unsafe"

	"github.com/ebitengine/purego"

	"hegel.dev/go/hegel/internal/cgo"
)

func newOutputFn(w io.Writer) (outputCallbackT, cgo.Handle) {
	if w != nil {
		return outputCallbackPtr, cgo.NewHandle(w)
	}
	return 0, 0
}

// withOutputFn extends free to release the writer's handle after the native
// object: the engine may write to the writer until then, including from the
// free itself.
func withOutputFn[T ~uintptr](free func(ctxT, T) Error, h cgo.Handle) func(ctxT, T) Error {
	if h == 0 {
		return free
	}
	return func(ctx ctxT, raw T) Error {
		e := free(ctx, raw)
		h.Delete()
		return e
	}
}

// deleteOutputFn releases the writer's handle when no native object took it.
func deleteOutputFn(h cgo.Handle) {
	if h != 0 {
		h.Delete()
	}
}

var outputCallbackPtr = outputCallbackT(purego.NewCallback(outputCallback))

// The return value is unused: hegel_output_callback_t is void-returning, so
// the engine ignores it. It exists only because purego.NewCallback maps to
// syscall.NewCallback on Windows, which requires exactly one uintptr result.
func outputCallback(userData uintptr, line *byte, length uint) uintptr {
	w := cgo.Handle(userData).Value().(io.Writer)
	// unsafe.String aliases the C buffer; io.WriteString copies the bytes into
	// the writer before this returns, so no separate copy is needed.
	_, _ = io.WriteString(w, unsafe.String(line, int(length)))
	_, _ = io.WriteString(w, "\n")
	return 0
}
