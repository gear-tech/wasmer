mod call_dynamic;
mod callback_signal;
mod chdir;
mod closure_allocate;
mod closure_free;
mod closure_prepare;
mod dl_invalid_handle;
mod dlopen;
mod dlsym;
mod epoll_create;
mod epoll_ctl;
mod epoll_wait;
mod fd_dup2;
mod fd_fdflags_get;
mod fd_fdflags_set;
mod fd_pipe;
mod futex_wait;
mod futex_wake;
mod futex_wake_all;
mod getcwd;
mod path_open2;
mod port_addr_add;
mod port_addr_clear;
mod port_addr_list;
mod port_addr_remove;
mod port_bridge;
mod port_dhcp_acquire;
mod port_gateway_set;
mod port_mac;
mod port_route_add;
mod port_route_clear;
mod port_route_list;
mod port_route_remove;
mod port_unbridge;
mod proc_exec;
mod proc_exec2;
mod proc_exec3;
mod proc_fork;
mod proc_id;
mod proc_join;
mod proc_parent;
mod proc_signal;
mod proc_signals_get;
mod proc_signals_sizes_get;
mod proc_snapshot;
mod proc_spawn;
mod proc_spawn2;
mod resolve;
mod sched_yield;
mod sock_accept;
mod sock_addr_local;
mod sock_addr_peer;
mod sock_bind;
mod sock_connect;
mod sock_get_opt_flag;
mod sock_get_opt_size;
mod sock_get_opt_time;
mod sock_join_multicast_v4;
mod sock_join_multicast_v6;
mod sock_leave_multicast_v4;
mod sock_leave_multicast_v6;
mod sock_listen;
mod sock_open;
mod sock_pair;
mod sock_recv;
mod sock_recv_from;
mod sock_send;
mod sock_send_file;
mod sock_send_to;
mod sock_set_opt_flag;
mod sock_set_opt_size;
mod sock_set_opt_time;
mod sock_shutdown;
mod sock_status;
mod stack_checkpoint;
mod stack_restore;
mod thread_exit;
mod thread_id;
mod thread_join;
mod thread_parallelism;
mod thread_signal;
mod thread_sleep;
mod thread_spawn;
mod tty_get;
mod tty_set;

pub use call_dynamic::*;
pub use callback_signal::*;
pub use chdir::*;
pub use closure_allocate::*;
pub use closure_free::*;
pub use closure_prepare::*;
pub use dl_invalid_handle::*;
pub use dlopen::*;
pub use dlsym::*;
pub use epoll_create::*;
pub use epoll_ctl::*;
pub use epoll_wait::*;
pub use fd_dup2::*;
pub use fd_fdflags_get::*;
pub use fd_fdflags_set::*;
pub use fd_pipe::*;
pub use futex_wait::*;
pub use futex_wake::*;
pub use futex_wake_all::*;
pub use getcwd::*;
pub use path_open2::*;
pub use port_addr_add::*;
pub use port_addr_clear::*;
pub use port_addr_list::*;
pub use port_addr_remove::*;
pub use port_bridge::*;
pub use port_dhcp_acquire::*;
pub use port_gateway_set::*;
pub use port_mac::*;
pub use port_route_add::*;
pub use port_route_clear::*;
pub use port_route_list::*;
pub use port_route_remove::*;
pub use port_unbridge::*;
pub use proc_exec::*;
pub use proc_exec2::*;
pub use proc_exec3::*;
pub use proc_fork::*;
pub use proc_id::*;
pub use proc_join::*;
pub use proc_parent::*;
pub use proc_signal::*;
pub use proc_signals_get::*;
pub use proc_signals_sizes_get::*;
pub use proc_snapshot::*;
pub use proc_spawn::*;
pub use proc_spawn2::*;
pub use resolve::*;
pub use sched_yield::*;
pub use sock_accept::*;
pub use sock_addr_local::*;
pub use sock_addr_peer::*;
pub use sock_bind::*;
pub use sock_connect::*;
pub use sock_get_opt_flag::*;
pub use sock_get_opt_size::*;
pub use sock_get_opt_time::*;
pub use sock_join_multicast_v4::*;
pub use sock_join_multicast_v6::*;
pub use sock_leave_multicast_v4::*;
pub use sock_leave_multicast_v6::*;
pub use sock_listen::*;
pub use sock_open::*;
pub use sock_pair::*;
pub use sock_recv::*;
pub use sock_recv_from::*;
pub use sock_send::*;
pub use sock_send_file::*;
pub use sock_send_to::*;
pub use sock_set_opt_flag::*;
pub use sock_set_opt_size::*;
pub use sock_set_opt_time::*;
pub use sock_shutdown::*;
pub use sock_status::*;
pub use stack_checkpoint::*;
pub use stack_restore::*;
pub use thread_exit::*;
pub use thread_id::*;
pub use thread_join::*;
pub use thread_parallelism::*;
pub use thread_signal::*;
pub use thread_sleep::*;
pub use thread_spawn::*;
pub use tty_get::*;
pub use tty_set::*;

use tracing::{debug_span, field, instrument, trace_span, Span};
