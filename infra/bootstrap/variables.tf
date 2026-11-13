variable "region" {
  description = "AWS region for the whole lab."
  type        = string
  default     = "ap-southeast-1"
}

variable "alert_email" {
  description = "Receives the budget emails. Set in ~/.miniarcade/bootstrap.tfvars, never in Git."
  type        = string

  validation {
    condition     = can(regex("^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$", var.alert_email))
    error_message = "alert_email must be an email address."
  }
}

variable "github_repository" {
  description = "The repository whose main branch may use the GitHub Actions role."
  type        = string
  default     = "watcharakrit-th/DevOpsVi"
}
