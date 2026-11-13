variable "region" {
  description = "AWS region for the whole lab."
  type        = string
  default     = "ap-southeast-1"
}

variable "my_ip" {
  description = "Your public IPv4 address, looked up by ops/up.sh. Only it may reach SSH and the Kubernetes API."
  type        = string

  validation {
    condition     = can(regex("^\\d{1,3}(\\.\\d{1,3}){3}$", var.my_ip)) && can(cidrhost("${var.my_ip}/32", 0))
    error_message = "my_ip must be an IPv4 address such as 203.0.113.7."
  }
}

variable "public_key_path" {
  description = "SSH public key for the node's ubuntu user. Set in ~/.miniarcade/cluster.tfvars."
  type        = string
}
