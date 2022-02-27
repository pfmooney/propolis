use structopt::StructOpt;

mod ioctl_helper;
mod print;

#[derive(Debug, StructOpt)]
#[structopt(name = "bhyveadm", about = "A stand-in for bhyvectl")]
struct Opt {
    /// Enable debugging
    #[structopt(long)]
    debug: bool,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Create new bhyve vmm instance
    Create {
        /// Instance name
        name: String,
        /// Instance will use VMM reservoir for guest memory allocations
        #[structopt(long)]
        use_reservoir: bool,
    },

    /// Destroy bhyve vmm instance
    Destroy {
        /// Instance name
        name: String,
    },

    Print {
        /// VMM instance name
        vm: String,
        /// Data components to print
        components: Vec<String>,
    },
    PrintCpu {
        /// VMM instance name
        vm: String,
        /// CPU ID
        vcpu: u32,
        /// Data components to print
        components: Vec<String>,
    },
    /// List components available to print
    ListComponents,
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);
    let res = match opt.cmd {
        Command::Create { name, use_reservoir } => {
            let mut flags = 0;
            if use_reservoir {
                flags |= bhyve_api::VCF_RESERVOIR_MEM;
            }
            ioctl_helper::create_vm(&name, flags)
        }
        Command::Destroy { name } => ioctl_helper::destroy_vm(&name),
        Command::Print { vm, components } => print::do_print(&vm, &components),
        Command::PrintCpu { vm, vcpu, components } => {
            print::do_print_cpu(&vm, vcpu, &components)
        }
        Command::ListComponents => print::component_list(),
    };
    res.unwrap();
}
