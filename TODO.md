1. Manual heap allocated buffers are leaking
2. Buffer pool
4. Program.init starts read on tty and read events emit keypress event or signal related events
5. Complete bindings for uv_loop_t, uv_errno_t, uv_tty_t, uv_stream_t, uv_handle_t, uv_timer_t, uv_signal_t
6. Program.run takes onupdate and onview callbacks
7. Program.run makes message type and processes accordingly
8. Program should be generic over the model type and message type
9. Find a way to handle lifetime on client
10. Model should be a trait with a 'view' method
11. After update runs it will do a dirty check on the model
    - if the model is dirty is will render (call 'view') and write the result
12. Remove View lifetime by changing to Box\[u8\]
