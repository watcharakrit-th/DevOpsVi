output "node_ip" {
  description = "The node's public IPv4 address."
  value       = aws_instance.node.public_ip
}
