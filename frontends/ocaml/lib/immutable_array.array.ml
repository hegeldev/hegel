type 'a t = 'a array

let init = Array.init

external get : 'a t -> int -> 'a = "%array_safe_get"
